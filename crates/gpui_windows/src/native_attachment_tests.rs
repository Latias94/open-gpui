use super::*;
use open_gpui::NoopTextSystem;
use open_gpui_wgpu::CosmicTextSystem;
use raw_window_handle::{HasWindowHandle, RawWindowHandle, Win32WindowHandle};
use std::{num::NonZeroIsize, ops::Range, sync::Once, time::Instant};
use windows::Win32::{
    System::Threading::GetCurrentThreadId,
    UI::Input::Ime::{HIMC, ImmAssociateContext, ImmGetContext, ImmReleaseContext},
};

const HOST_USERDATA: isize = 0x123456;
const HOST_MESSAGE: u32 = WM_APP + 73;

struct HostWindow(HWND);

impl HostWindow {
    fn new() -> Result<Rc<Self>> {
        static REGISTER: Once = Once::new();
        let instance = unsafe { GetModuleHandleW(None) }?;
        REGISTER.call_once(|| {
            let class = WNDCLASSW {
                lpfnWndProc: Some(host_window_proc),
                hInstance: instance.into(),
                lpszClassName: w!("OpenGPUI::AttachmentTestHost"),
                ..Default::default()
            };
            assert_ne!(unsafe { RegisterClassW(&class) }, 0);
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW,
                w!("OpenGPUI::AttachmentTestHost"),
                w!("Application-owned native attachment"),
                WS_OVERLAPPEDWINDOW,
                40,
                40,
                320,
                240,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }?;
        unsafe { set_window_long(hwnd, GWLP_USERDATA, HOST_USERDATA) };
        Ok(Rc::new(Self(hwnd)))
    }
}

impl HasWindowHandle for HostWindow {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        let mut raw = Win32WindowHandle::new(NonZeroIsize::new(self.0.0 as isize).unwrap());
        raw.hinstance = NonZeroIsize::new(unsafe { get_window_long(self.0, GWLP_HINSTANCE) });
        // SAFETY: HostWindow owns this HWND and destroys it only after the borrow ends.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw.into()) })
    }
}

impl Drop for HostWindow {
    fn drop(&mut self) {
        if unsafe { IsWindow(Some(self.0)).as_bool() } {
            unsafe { DestroyWindow(self.0) }.expect("host destroys its original HWND");
        }
    }
}

unsafe extern "system" fn host_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        HOST_MESSAGE => LRESULT(unsafe { get_window_long(hwnd, GWLP_USERDATA) }),
        WM_TIMER if wparam.0 == 1 => LRESULT(91),
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn pump_until(platform: &WindowsPlatform, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "native attachment did not converge"
        );
        let mut message = MSG::default();
        if unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            assert_ne!(
                message.message, WM_QUIT,
                "GPUI posted a host-owned quit message"
            );
            if !platform.pretranslate_message(&message) {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

struct RejectingFactory;

impl WindowsRendererFactory for RejectingFactory {
    fn create(&self, _: WindowsRendererWindow) -> Result<Box<dyn WindowsRenderer>> {
        anyhow::bail!("injected native attachment renderer creation failure")
    }
}

#[core::prelude::v1::test]
fn native_external_host_returns_launch_and_preserves_quit_under_foreground_load() -> Result<()> {
    let platform = Rc::new(WindowsPlatform::new_external_host(
        Arc::new(NoopTextSystem::new()),
        Rc::new(RejectingFactory),
    )?);
    let launched = Rc::new(Cell::new(0));
    let launch = launched.clone();
    let app = Application::with_platform(platform.clone())
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(move |_| launch.set(launch.get() + 1));
    assert_eq!(launched.get(), 1);
    let task = platform.foreground_executor.spawn(async {
        std::thread::sleep(Duration::from_millis(12));
    });
    task.detach();
    unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_QUIT, WPARAM(73), LPARAM(0)) }?;
    platform.inner.run_foreground_task();
    let mut message = MSG::default();
    assert!(unsafe { PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_REMOVE) }.as_bool());
    assert_eq!(message.message, WM_QUIT);
    assert_eq!(message.wParam.0, 73);
    app.update(|cx| cx.quit());
    pump_until(&platform, || platform.is_shutdown_complete());
    Ok(())
}

#[derive(Default)]
struct RendererObservation {
    reject_creation: Cell<bool>,
    draws: Cell<usize>,
    shutdowns: RefCell<Vec<WindowPresentationShutdownTicket>>,
}

struct AttachmentFactory {
    devices: DirectXDevices,
    observation: Rc<RendererObservation>,
}

impl WindowsRendererFactory for AttachmentFactory {
    fn create(&self, window: WindowsRendererWindow) -> Result<Box<dyn WindowsRenderer>> {
        anyhow::ensure!(
            !self.observation.reject_creation.get(),
            "injected attachment creation failure"
        );
        let RawWindowHandle::Win32(handle) = window.raw_window_handle() else {
            unreachable!();
        };
        let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
        assert_eq!(
            handle.hinstance,
            NonZeroIsize::new(unsafe { get_window_long(hwnd, GWLP_HINSTANCE) })
        );
        Ok(Box::new(AttachmentRenderer {
            renderer: DirectXRenderer::new(hwnd, &self.devices, true)?,
            observation: self.observation.clone(),
            refuse_quiescence: Cell::new(true),
        }))
    }
}

struct AttachmentRenderer {
    renderer: DirectXRenderer,
    observation: Rc<RendererObservation>,
    refuse_quiescence: Cell<bool>,
}

impl WindowsRenderer for AttachmentRenderer {
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.renderer.sprite_atlas()
    }

    fn resize(&mut self, size: Size<DevicePixels>) -> Result<()> {
        self.renderer.resize(size)
    }

    fn draw(
        &mut self,
        scene: &Scene,
        background: WindowBackgroundAppearance,
    ) -> Result<PlatformWindowPresentOutcome> {
        self.observation.draws.set(self.observation.draws.get() + 1);
        WindowsRenderer::draw(&mut self.renderer, scene, background)
    }

    fn quiesce(&mut self, shutdown: &WindowPresentationShutdownTicket) -> Result<()> {
        self.observation
            .shutdowns
            .borrow_mut()
            .push(shutdown.clone());
        anyhow::ensure!(
            !self.refuse_quiescence.replace(false),
            "injected first quiescence refusal"
        );
        self.renderer.quiesce_surface(shutdown)
    }
}

struct AttachmentRoot;

impl Render for AttachmentRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x264568))
    }
}

#[core::prelude::v1::test]
fn native_attachment_preserves_host_services_and_retires_exact_binding_after_retry() -> Result<()> {
    let owner = HostWindow::new()?;
    let hwnd = owner.0;
    let original_ime = unsafe { ImmGetContext(hwnd) };
    if !original_ime.is_invalid() {
        unsafe { ImmReleaseContext(hwnd, original_ime) }.ok()?;
    }
    assert_ne!(unsafe { SetTimer(Some(hwnd), 1, 60_000, None) }, 0);
    let observation = Rc::new(RendererObservation::default());
    let platform = Rc::new(WindowsPlatform::new_external_host(
        Arc::new(CosmicTextSystem::new("Segoe UI")),
        Rc::new(AttachmentFactory {
            devices: DirectXDevices::new()?,
            observation: observation.clone(),
        }),
    )?);
    let app = Application::with_platform(platform.clone())
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| {});
    let open = || {
        app.update(|cx| {
            cx.open_window_with_platform_window(
                WindowOptions {
                    show: false,
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |handle, params, displays| {
                    platform.attach_window(handle, params, displays, owner.clone())
                },
                |_, cx| cx.new(|_| AttachmentRoot),
            )
        })
    };
    observation.reject_creation.set(true);
    assert!(open().is_err());
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert_eq!(
        unsafe { get_window_long(hwnd, GWLP_USERDATA) },
        HOST_USERDATA
    );
    assert!(platform.window_from_hwnd(hwnd).is_none());
    observation.reject_creation.set(false);
    let first = open()?;
    let native = platform
        .window_from_hwnd(hwnd)
        .expect("exact attached native owner");
    let registration = native.registration;
    assert_eq!(registration.window_id(), first.window_id());
    assert_eq!(Rc::strong_count(&owner), 2);
    assert_eq!(
        unsafe { SendMessageW(hwnd, HOST_MESSAGE, None, None) }.0,
        HOST_USERDATA
    );
    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_TIMER, Some(WPARAM(1)), None) }.0,
        91
    );
    unsafe { SendMessageW(hwnd, WM_CLOSE, None, None) };
    let shutdown = native.presentation_shutdown_ticket();
    pump_until(&platform, || shutdown.snapshot().detached());
    assert!(!shutdown.snapshot().native_terminal());
    assert!(shutdown.snapshot().quiesced());
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(unsafe { IsWindowEnabled(hwnd).as_bool() });
    assert_eq!(Rc::strong_count(&owner), 1);
    assert!(platform.window_from_hwnd(hwnd).is_none());
    assert!(observation.shutdowns.borrow().len() >= 2);
    assert!(
        observation
            .shutdowns
            .borrow()
            .iter()
            .all(|attempt| attempt.same_authority(&shutdown))
    );
    let restored_ime = unsafe { ImmGetContext(hwnd) };
    assert_eq!(restored_ime, original_ime);
    if !restored_ime.is_invalid() {
        unsafe { ImmReleaseContext(hwnd, restored_ime) }.ok()?;
    }
    let second = open()?;
    let replacement = platform
        .window_from_hwnd(hwnd)
        .expect("replacement native binding");
    assert_ne!(
        replacement.registration.generation(),
        registration.generation()
    );
    let binding = WindowsWindow(replacement.clone());
    assert!(!replacement.presentation_shutdown_claimed());
    assert_eq!(
        binding.retire_native_window(&shutdown),
        PlatformNativeWindowRetirementOutcome::Rejected
    );
    assert!(!replacement.presentation_shutdown_claimed());
    assert!(platform.window_from_hwnd(hwnd).is_some());
    assert_eq!(Rc::strong_count(&owner), 2);
    let draws = observation.draws.get();
    let frame_requests = Rc::new(Cell::new(0));
    let original_request = replacement
        .state
        .callbacks
        .request_frame
        .take()
        .expect("the live replacement must retain its original App frame callback");
    replacement.state.callbacks.request_frame.set(Some(Rc::new({
        let frame_requests = frame_requests.clone();
        let original_request = original_request.clone();
        move |options| {
            frame_requests.set(frame_requests.get() + 1);
            original_request(options);
        }
    })));
    unsafe {
        SendMessageW(
            hwnd,
            WM_GPUI_FORCE_UPDATE_WINDOW,
            Some(WPARAM(platform.inner.validation_number)),
            Some(LPARAM(registration.generation() as isize)),
        );
    }
    assert_eq!(observation.draws.get(), draws);
    assert_eq!(frame_requests.get(), 0);
    unsafe {
        SendMessageW(
            hwnd,
            WM_GPUI_FORCE_UPDATE_WINDOW,
            Some(WPARAM(platform.inner.validation_number)),
            Some(LPARAM(replacement.registration.generation() as isize)),
        );
    }
    assert_eq!(frame_requests.get(), 1);
    replacement
        .state
        .callbacks
        .request_frame
        .set(Some(original_request));
    let current = replacement.presentation_shutdown_ticket();
    let foreign = WindowPresentationShutdownTicket::new(
        current.snapshot().window_id(),
        current.snapshot().generation(),
    );
    assert!(foreign.acknowledge_quiesced());
    assert!(foreign.acknowledge_detached());
    assert!(!foreign.same_authority(&current));
    assert_eq!(
        binding.retire_native_window(&foreign),
        PlatformNativeWindowRetirementOutcome::Rejected
    );
    assert_eq!(
        binding.retire_native_window(&shutdown),
        PlatformNativeWindowRetirementOutcome::Rejected
    );
    assert!(
        replacement
            .presentation_shutdown_ticket()
            .same_authority(&current)
    );
    assert!(!current.snapshot().quiesced());
    assert!(!current.snapshot().binding_terminal());
    app.update(|cx| second.update(cx, |_, window, cx| window.remove_window(cx)))?;
    pump_until(&platform, || platform.window_from_hwnd(hwnd).is_none());
    assert!(current.snapshot().detached());
    assert_eq!(
        binding.retire_native_window(&current),
        PlatformNativeWindowRetirementOutcome::BindingDetached
    );
    drop(binding);
    unsafe { KillTimer(Some(hwnd), 1) }?;
    assert_eq!(
        unsafe { SendMessageW(hwnd, HOST_MESSAGE, None, None) }.0,
        HOST_USERDATA
    );
    app.update(|cx| cx.quit());
    pump_until(&platform, || platform.is_shutdown_complete());
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    Ok(())
}

struct AcceptingTextHandler(Rc<Cell<usize>>);

#[core::prelude::v1::test]
fn native_external_host_releases_owned_window_input_services_before_ole_shutdown() -> Result<()> {
    let platform = Rc::new(WindowsPlatform::new_external_host(
        Arc::new(CosmicTextSystem::new("Segoe UI")),
        Rc::new(AttachmentFactory {
            devices: DirectXDevices::new()?,
            observation: Rc::new(RendererObservation::default()),
        }),
    )?);
    let app = Application::with_platform(platform.clone())
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| {});
    let handle = app.update(|cx| {
        cx.open_window(
            WindowOptions {
                show: false,
                focus_on_appearing: false,
                ..Default::default()
            },
            |_, cx| cx.new(|_| AttachmentRoot),
        )
    })?;
    let registration = platform
        .raw_window_handles
        .read()
        .iter()
        .find(|registration| registration.window_id() == handle.window_id())
        .copied()
        .expect("ordinary opening must retain its exact native registration");
    let native = platform
        .window_from_hwnd(registration.as_raw())
        .expect("the owned window must remain available before shutdown");
    assert!(!native.state.direct_manipulation.is_retired());
    app.update(|cx| cx.quit());
    pump_until(&platform, || platform.is_shutdown_complete());
    assert!(!unsafe { IsWindow(Some(registration.as_raw())) }.as_bool());
    assert!(native.state.direct_manipulation.is_retired());
    assert!(
        native
            .presentation_shutdown_ticket()
            .snapshot()
            .native_terminal()
    );
    drop(native);
    Ok(())
}

impl InputHandler for AcceptingTextHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        Some(String::new())
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        None
    }

    fn accepts_text_input(&mut self, _: &mut Window, _: &mut App) -> bool {
        self.0.set(self.0.get() + 1);
        true
    }
}

struct ImeRoot {
    focus: FocusHandle,
    queries: Rc<Cell<usize>>,
}

impl Render for ImeRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let queries = self.queries.clone();
        div().size_full().track_focus(&self.focus).child(
            canvas(
                |_, _, _| (),
                move |_, (), window, cx| {
                    window.handle_input(&focus, AcceptingTextHandler(queries), cx)
                },
            )
            .size_full(),
        )
    }
}

#[core::prelude::v1::test]
fn native_attachment_enables_first_text_handler_from_null_himc_and_restores_null() -> Result<()> {
    let owner = HostWindow::new()?;
    let hwnd = owner.0;
    unsafe { ImmAssociateContext(hwnd, HIMC::default()) };
    assert!(unsafe { ImmGetContext(hwnd) }.is_invalid());
    let platform = Rc::new(WindowsPlatform::new_external_host(
        Arc::new(CosmicTextSystem::new("Segoe UI")),
        Rc::new(AttachmentFactory {
            devices: DirectXDevices::new()?,
            observation: Rc::new(RendererObservation::default()),
        }),
    )?);
    let app = Application::with_platform(platform.clone())
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(|_| {});
    let initial_ime = Cell::new(None);
    let queries = Rc::new(Cell::new(0));
    let handle = app.update(|cx| {
        cx.open_window_with_platform_window(
            WindowOptions {
                show: false,
                focus_on_appearing: false,
                ..Default::default()
            },
            |handle, params, displays| {
                let window = platform.attach_window(handle, params, displays, owner.clone())?;
                initial_ime.set(Some(
                    platform
                        .window_from_hwnd(hwnd)
                        .unwrap()
                        .state
                        .ime_enabled
                        .get(),
                ));
                Ok(window)
            },
            |window, cx| {
                let root = cx.new(|cx| ImeRoot {
                    focus: cx.focus_handle(),
                    queries: queries.clone(),
                });
                let focus = root.read(cx).focus.clone();
                window.focus(&focus, cx);
                root
            },
        )
    })?;
    assert_eq!(initial_ime.get(), Some(false));
    let native = platform
        .window_from_hwnd(hwnd)
        .expect("focused native text binding");
    assert_eq!(
        native.complete_native_frame(),
        PlatformWindowCommandOutcome::Accepted
    );
    assert!(queries.get() > 0);
    let associated = unsafe { ImmGetContext(hwnd) };
    assert!(
        !associated.is_invalid(),
        "first focused text handler must associate the default HIMC"
    );
    unsafe { ImmReleaseContext(hwnd, associated) }.ok()?;
    app.update(|cx| handle.update(cx, |_, window, cx| window.remove_window(cx)))?;
    pump_until(&platform, || platform.window_from_hwnd(hwnd).is_none());
    assert!(
        unsafe { ImmGetContext(hwnd) }.is_invalid(),
        "detachment must restore the host's null HIMC"
    );
    app.update(|cx| cx.quit());
    pump_until(&platform, || platform.is_shutdown_complete());
    Ok(())
}
