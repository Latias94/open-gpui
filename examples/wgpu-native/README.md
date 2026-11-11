# Native WGPU consumer

This Windows example owns its WGPU instance, adapter, device, queue, surface and imported image.
One factory and renderer serve all modes. The renderer records the image producer before GPUI
samples that image and retains each `WgpuSceneRecording` until its original GPU completion prefix
finishes. Synchronous delivery submits and presents before returning `Submitted`. Deferred delivery
retains the real finished command buffer, acquired surface texture, scene recording and original
`PlatformFrameSubmission`; the host decides when to submit after the draw callback returns.
Renderer quiescence discards unsubmitted work, waits for the original submitted prefix and releases
the surface before native binding retirement. This is an ordinary application using production
APIs, without `test-support`, `bench-support`, or a simulated/headless Window.

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

Use the same external host with actual deferred GPU submission:

```text
cargo run --locked -p open-gpui-wgpu-native -- --external-host --deferred
cargo run --locked -p open-gpui-wgpu-native -- --external-host --deferred --smoke
```

`--deferred` requires `--external-host`. Interactive deferred mode drains the pending packet on the
next host loop turn. The deferred smoke holds packets until its current acceptance phase permits
actual submission. Both paths drain on the UI thread outside `ApplicationHandle::update`.

The external host registers its own window class and WndProc, creates a hidden top-level HWND,
and supplies an `Rc<HostWindow>` implementing `HasWindowHandle`, including the actual HINSTANCE.
It attaches through `App::open_window_with_platform_window` and
`WindowsPlatform::attach_window`. It retains the `ApplicationHandle` returned by
`Application::run_embedded` throughout shutdown. Pretranslation, native dispatch and native
geometry changes run outside any active App update. GPUI's existing dispatcher handles its
own wakes; the host blocks in `GetMessageW` when the queue is idle.

Initial visibility belongs to GPUI's normal initial-presentation command. The host never manually
shows the pending initial window. The deferred smoke checks the real HWND remains hidden with no
accepted/submitted facts or committed button geometry, excludes it from a native pointer query,
and verifies native `WM_CHAR` cannot reach the unsubmitted text receiver. It then submits that
original hidden packet and waits for `WindowInitialPresentationStatus::Completed` and native
visibility. Later explicit minimize/hide/restore controls run only after that completion.

Each packet freezes its producer count, frame/attempt identity, physical extent and native target
basis at recording time. Submit never reads the latest producer count. Before `queue.submit`, the
host checks the original token's `is_pending()` (including framework font/atlas validity), the
renderer target generation, and the queried HWND origin, extent, DPI, visibility and iconic state.
Cancelled or superseded packets are dropped without a queue submission. Resize releases the
unsubmitted acquired texture before surface reconfiguration; the target generation also prevents
a same-size resize ABA from retargeting an old packet. Only after `queue.submit` and presentation
does the host complete the original token as `Submitted`. That completion reports presentation
submission; the separate GPU callback prefix still controls recording retention and quiescence.

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
The deferred smoke additionally checks, in order:

- Hidden initial recording, absent initial publications/effects, actual submission of the original
  packet, then the normal completed initial-show gate.
- Visible submitted S0, candidate A with a moved button and another ordinary text receiver, and a
  live source change to B while A remains pending. Actual A submission must retain A's recorded
  image count and exact generation; candidate B cannot publish before its own actual submission.
  Each original A/B encoder also copies one producer RGBA8 pixel to its own readback buffer.
  After that same queue submission, the smoke waits on its original `SubmissionIndex` and checks
  distinct expected A/B pixels. This verifies frozen producer inputs, not swapchain output.
- Actual submission and publication of the original B packet, followed by a further pending
  candidate with another receiver and button position. Real button and UTF-16 messages must keep
  using B's last-submitted button and text receiver; the candidate receiver remains untouched.
  Input may cancel that candidate; stale original-token completion must return `false`, followed
  by an actual fresh-candidate submission.
- Real move/resize invalidation releasing the original recording before submission, an intervening
  resized candidate, and restoration of the original extent. Neither superseded token may be
  completed or retargeted through this same-size ABA; only the fresh packet is submitted.
- Explicit `original_token.complete(Rejected)` before any queue submission, terminal stale-token
  refusal, unchanged submitted publications, and an actually submitted replacement attempt.
- Real minimized and hidden `Presented` states established before new frame demand: no provider
  offers or GPU submissions while blocked, and new original packets actually submitted after each
  restore. This exercises the adapter's completion of attempts it cannot offer to the provider.
- All existing synchronous smoke assertions, reused with deferred host draining: idle blocking,
  normal button/F6/BMP and surrogate-pair text, native/GPUI/renderer geometry, and retained GPU work.
- A real `WM_CLOSE` with another original packet still unsubmitted. Shutdown must invalidate and
  drop it without changing the submitted prefix, quiesce that exact original prefix, and report
  typed detachment. The host continues its private-message and `WM_QUIT` checks before destruction.
  The original submitted recording remains retained through the quiesce wait. Close and quiesce
  snapshots report its exact prefix, retained recordings and observed callback completion before
  and after waiting; an unobserved completion does not establish that hardware is still executing.

The existing synchronous external smoke keeps its 20 s acceptance deadline. The expanded deferred
trajectory has a 40 s overall deadline and also retains the reused smoke's own 20 s deadline.
Launch/initial-presentation and shutdown waits remain bounded at 15 s. GPU waits remain bounded.
Scripted native queries/messages do not qualify physical keyboard input, IME composition, or a
physical monitor/DPI transition. This consumer claims one ordinary top-level HWND; it does not
claim logical roots, child-window APIs, reattachment, or retired-identity behavior. The latter
controls belong in native adapter tests.

Integration requires the matching Core and native adapter deferred APIs: original-window candidate
custody and completion ingress, an initial command held until its original submission completes,
`WindowsRenderer::frame_delivery_mode`/`draw_with_submission`, token invalidation before native
target changes/shutdown, and `RepaintRequired` completion for iconic/hidden Presented attempts
that never reach the provider. The consumer revalidates `is_pending()` immediately before actual
submission. Core/CPU and native-adapter unit qualification remain separate from this executable
GPU acceptance; run all three smoke commands above to qualify the combined implementation.
