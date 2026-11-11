# Native WGPU consumer

This Windows example owns its WGPU instance, adapter, device, queue, surface and imported image.
The same renderer records the image producer before GPUI samples that image, submits the real
command buffer before returning `Submitted`, and retains each `WgpuSceneRecording` until its
original GPU completion prefix finishes. Renderer quiescence waits for that work and releases
the surface before native binding retirement.

Run the ordinary GPUI-owned window and event loop:

```text
cargo run --locked -p open-gpui-wgpu-native
cargo run --locked -p open-gpui-wgpu-native -- --smoke
```

Run with an application-created HWND and application-owned Win32 message loop:

```text
cargo run --locked -p open-gpui-wgpu-native -- --external-host
cargo run --locked -p open-gpui-wgpu-native -- --external-host --smoke
```

The external host registers its own window class and WndProc, creates a hidden top-level HWND,
and supplies an `Rc<HostWindow>` implementing `HasWindowHandle`, including the actual HINSTANCE.
It attaches through `App::open_window_with_platform_window` and
`WindowsPlatform::attach_window`. It retains the `ApplicationHandle` returned by
`Application::run_embedded` throughout shutdown. Pretranslation, native dispatch and native
geometry changes run outside any active App update. GPUI's existing dispatcher handles its
own wakes; the host blocks in `GetMessageW` when the queue is idle.

Click **Change GPU image**, type into the text receiver, or press **F6** to change the image.
Close the window to request normal GPUI shutdown. The application keeps pumping until
`WindowsPlatform::is_shutdown_complete`, checks that its HWND and userdata survived, processes
another private host message, and only then destroys its own window and ends its message loop.
The text receiver demonstrates native delivery and a field-level text-service anchor; it is
not a full editor with caret layout or pointer selection.

The external smoke uses actual Win32 messages and native queries to check:

- Initial and changed GPU submissions through the shared renderer, including normal button
  clicks and F6 routed through native pretranslation.
- Native `WM_CHAR` delivery of BMP and surrogate-pair text to GPUI's regular input handler.
- A bounded idle interval with measured message count and blocking time.
- A real move and resize via `SetWindowPos`, followed by agreement among queried native client
  bounds, GPUI bounds and viewport, renderer configuration, and the submitted frame's extent.
- Normal quit requested with real submitted GPU work awaiting completion. The actual shutdown
  ticket must report quiescence and `detached`, with no native destruction or protocol violation.
- An alive, enabled HWND with unchanged application userdata and HINSTANCE after GPUI shutdown,
  and delivery of a private message after detachment.
- Preservation of a host-owned `WM_QUIT` marker queued before shutdown, refusal to accept any
  unexpected adapter-generated quit, and retrieval of the host's final quit after its own
  `DestroyWindow` call.

Smoke deadlines and application, renderer, or GPU errors exit nonzero. Its 25 ms host timer
bounds acceptance and teardown waits; interactive idle use removes that timer after launch.
Scripted message routing does not qualify physical keyboard input, IME composition, or a
physical monitor/DPI transition. Reattachment and retired-identity controls belong in native
adapter tests rather than fabricated child-window APIs in this consumer.
