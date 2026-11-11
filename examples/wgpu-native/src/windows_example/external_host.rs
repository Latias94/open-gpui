//! An ordinary application-owned HWND and message loop, without native test facilities.
//!
//! The host retains its HWND owner and ApplicationHandle through native binding retirement.
//! Pretranslation and native dispatch run outside App updates. PeekMessage only observes queue
//! availability; GetMessage owns retrieval and blocks when idle. The host timer bounds smoke
//! and shutdown waits without driving GPUI's scheduler; interactive idle use removes it.
//!
//! Smoke queues a host-owned WM_QUIT marker before shutdown to detect accidental consumption.
//! After GPUI detaches and another private message arrives, only the host destroys the HWND
//! and posts its final WM_QUIT. Failed acceptance exits nonzero without dropping an attached owner.

mod deferred_smoke;
mod input;
mod smoke;

use super::*;
use anyhow::Context as _;
use input::HostInput;
use open_gpui::{ApplicationHandle, Entity, WindowHandle, WindowInitialPresentationStatus};
use raw_window_handle::{HandleError, HasWindowHandle, Win32WindowHandle};
use std::{num::NonZeroIsize, time::Instant};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{Input::KeyboardAndMouse::IsWindowEnabled, WindowsAndMessaging::*},
    },
    core::w,
};

const HOST_MESSAGE: u32 = WM_APP + 0x217;
const HOST_INPUT_FENCE: u32 = WM_APP + 0x218;
const HOST_TIMER: usize = 0x4e47;
const HOST_SENTINEL: u64 = 0x4e41_5241_484f_5354;
const HOST_QUIT_PROBE: usize = 0x4e47_5055;
const CLASS_NAME: windows::core::PCWSTR = w!("OpenGpuiWgpuExternalHost");

struct HostWindow {
    hwnd: Cell<HWND>,
    instance: HINSTANCE,
    sentinel: u64,
    private_messages: Cell<u32>,
    last_private: Cell<usize>,
    last_input_fence: Cell<usize>,
    timer_ticks: Cell<u32>,
    close_requested: Cell<bool>,
    ending_loop: Cell<bool>,
    destroyed: Cell<bool>,
}

impl HostWindow {
    fn new() -> Result<Rc<Self>> {
        // SAFETY: All native window operations run on this application's UI thread.
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }?.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(host_window_proc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }?,
            ..Default::default()
        };
        ensure!(
            unsafe { RegisterClassW(&class) } != 0,
            "registering the host window class: {}",
            windows::core::Error::from_thread()
        );
        let host = Rc::new(Self {
            hwnd: Cell::new(HWND::default()),
            instance,
            sentinel: HOST_SENTINEL,
            private_messages: Cell::new(0),
            last_private: Cell::new(0),
            last_input_fence: Cell::new(0),
            timer_ticks: Cell::new(0),
            close_requested: Cell::new(false),
            ending_loop: Cell::new(false),
            destroyed: Cell::new(false),
        });
        // SAFETY: Rc gives the WndProc a stable address. The host retains it until after
        // DestroyWindow returns; the GPUI adapter retains another clone through detachment.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                CLASS_NAME,
                w!("Application-owned Win32 host + WGPU + GPUI"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                720,
                540,
                None,
                None,
                Some(instance),
                Some(Rc::as_ptr(&host).cast()),
            )
        }
        .context("creating the application's HWND")?;
        ensure!(
            host.hwnd.get() == hwnd,
            "the application's WndProc did not receive WM_NCCREATE"
        );
        host.verify()?;
        Ok(host)
    }

    fn verify(&self) -> Result<()> {
        let hwnd = self.hwnd.get();
        // SAFETY: These queries do not dispatch callbacks or borrow the GPUI App.
        ensure!(
            unsafe { IsWindow(Some(hwnd)) }.as_bool(),
            "GPUI destroyed the host's HWND"
        );
        ensure!(
            unsafe { IsWindowEnabled(hwnd) }.as_bool(),
            "the host's HWND was left disabled"
        );
        ensure!(
            !self.destroyed.get(),
            "the host's WndProc received premature WM_DESTROY"
        );
        ensure!(
            self.sentinel == HOST_SENTINEL,
            "the host userdata sentinel changed"
        );
        ensure!(
            unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } == self as *const Self as isize,
            "GPUI replaced the application's GWLP_USERDATA"
        );
        ensure!(
            unsafe { GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) } == self.instance.0 as isize,
            "the host HWND's actual HINSTANCE changed"
        );
        Ok(())
    }

    fn post_private(&self, cookie: usize) -> Result<()> {
        // SAFETY: The message carries only an integer owned and interpreted by the host.
        unsafe {
            PostMessageW(
                Some(self.hwnd.get()),
                HOST_MESSAGE,
                WPARAM(cookie),
                LPARAM(0),
            )
        }?;
        Ok(())
    }

    fn start_timer(&self) -> Result<()> {
        // SAFETY: The timer belongs to the host's live HWND and has no raw callback pointer.
        ensure!(
            unsafe { SetTimer(Some(self.hwnd.get()), HOST_TIMER, 25, None) } != 0,
            "creating the host's smoke/shutdown watchdog"
        );
        Ok(())
    }

    fn post_input_fence(&self, cookie: usize) -> Result<()> {
        // SAFETY: A host-owned integer receipt follows the scripted input in the same queue.
        unsafe {
            PostMessageW(
                Some(self.hwnd.get()),
                HOST_INPUT_FENCE,
                WPARAM(cookie),
                LPARAM(0),
            )
        }?;
        Ok(())
    }

    fn client_extent(&self) -> Result<(i32, i32)> {
        let mut rect = RECT::default();
        // SAFETY: rect is valid writable storage and this is the host's live HWND.
        unsafe { GetClientRect(self.hwnd.get(), &mut rect) }?;
        Ok((rect.right - rect.left, rect.bottom - rect.top))
    }

    fn destroy(&self) -> Result<()> {
        self.verify()?;
        self.ending_loop.set(true);
        // SAFETY: Only the host destroys its HWND, after GPUI's shutdown-complete fence.
        unsafe { DestroyWindow(self.hwnd.get()) }.context("destroying the host's own HWND")?;
        ensure!(
            self.destroyed.get(),
            "the host did not receive its own WM_DESTROY"
        );
        ensure!(
            !unsafe { IsWindow(Some(self.hwnd.get())) }.as_bool(),
            "the host's DestroyWindow did not destroy its HWND"
        );
        Ok(())
    }
}

impl HasWindowHandle for HostWindow {
    fn window_handle(
        &self,
    ) -> std::result::Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        if self.destroyed.get() {
            return Err(HandleError::Unavailable);
        }
        let hwnd = NonZeroIsize::new(self.hwnd.get().0 as isize).ok_or(HandleError::Unavailable)?;
        let mut raw = Win32WindowHandle::new(hwnd);
        raw.hinstance = NonZeroIsize::new(self.instance.0 as isize);
        // SAFETY: The host and adapter retain Rc owners until renderer quiescence and
        // detachment finish. Only the host then calls DestroyWindow on its owning thread.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw.into()) })
    }
}

impl Drop for HostWindow {
    fn drop(&mut self) {
        // SAFETY: The adapter's Rc prevents this fallback from running during attachment.
        // It covers errors before launch; the success path explicitly destroys the window.
        unsafe {
            if IsWindow(Some(self.hwnd.get())).as_bool() {
                let _ = DestroyWindow(self.hwnd.get());
            }
            let _ = UnregisterClassW(CLASS_NAME, Some(self.instance));
        }
    }
}

unsafe extern "system" fn host_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // SAFETY: CreateWindowExW supplies CREATESTRUCTW for WM_NCCREATE. lpCreateParams is
        // the Rc-stable HostWindow address supplied by HostWindow::new on this same thread.
        let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let host = unsafe { &*create.lpCreateParams.cast::<HostWindow>() };
        host.hwnd.set(hwnd);
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, host as *const HostWindow as isize) };
    }
    // SAFETY: This slot belongs to this application's WndProc and survives GPUI attachment.
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const HostWindow;
    if let Some(host) = unsafe { pointer.as_ref() } {
        match message {
            HOST_MESSAGE => {
                host.private_messages
                    .set(host.private_messages.get().wrapping_add(1));
                host.last_private.set(wparam.0);
                return LRESULT(0);
            }
            HOST_INPUT_FENCE => {
                host.last_input_fence.set(wparam.0);
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == HOST_TIMER => {
                host.timer_ticks.set(host.timer_ticks.get().wrapping_add(1));
                return LRESULT(0);
            }
            WM_CLOSE => {
                host.close_requested.set(true);
                return LRESULT(0);
            }
            WM_DESTROY => {
                host.destroyed.set(true);
                if host.ending_loop.get() {
                    unsafe { PostQuitMessage(0) };
                }
            }
            WM_NCDESTROY => {
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            }
            _ => {}
        }
    }
    // SAFETY: Unhandled native messages keep their ordinary host/default behavior.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

#[derive(Default)]
struct LoopObservation {
    messages: u64,
    blocked: Duration,
    quit_probe_seen: bool,
}

enum HostSmoke {
    Synchronous(smoke::Smoke),
    Deferred(deferred_smoke::Smoke),
}

impl HostSmoke {
    fn auto_drain(&self) -> bool {
        matches!(self, Self::Deferred(smoke) if smoke.auto_drain())
    }

    fn advance(
        &mut self,
        host: &HostWindow,
        application: &ApplicationHandle,
        factory: &Factory,
        window: Option<&WindowHandle<Root>>,
        input: &RefCell<Option<Entity<HostInput>>>,
        observation: &LoopObservation,
    ) -> Result<()> {
        match self {
            Self::Synchronous(smoke) => {
                smoke.advance(host, application, factory, window, input, observation)
            }
            Self::Deferred(smoke) => {
                smoke.advance(host, application, factory, window, input, observation)
            }
        }
    }

    fn ready_to_shutdown(&self) -> bool {
        match self {
            Self::Synchronous(smoke) => smoke.ready_to_shutdown(),
            Self::Deferred(smoke) => smoke.ready_to_shutdown(),
        }
    }

    fn verify_detached(&self, factory: &Factory) -> Result<()> {
        match self {
            Self::Synchronous(smoke) => smoke.verify_detached(factory),
            Self::Deferred(smoke) => smoke.verify_detached(factory),
        }
    }

    fn report(&self, factory: &Factory, observation: &LoopObservation) {
        match self {
            Self::Synchronous(smoke) => smoke.report(factory, observation),
            Self::Deferred(smoke) => smoke.report(factory, observation),
        }
    }
}

pub(super) fn run(factory: Rc<Factory>, smoke: bool) -> Result<()> {
    let platform = Rc::new(WindowsPlatform::new_external_host(
        Arc::new(CosmicTextSystem::new("Segoe UI")),
        factory.clone(),
    )?);
    let host = HostWindow::new()?;
    let opened = Rc::new(RefCell::new(None));
    let input = Rc::new(RefCell::new(None));
    let application = Application::with_platform(platform.clone()).run_embedded({
        let platform = platform.clone();
        let host = host.clone();
        let factory = factory.clone();
        let opened = opened.clone();
        let input = input.clone();
        move |cx| {
            let result = cx
                .open_window_with_platform_window(
                    WindowOptions::default(),
                    move |handle, params, displays| {
                        platform.attach_window(handle, params, displays, host)
                    },
                    |window, cx| {
                        let receiver = cx.new(|cx| HostInput::new(factory.count.clone(), cx));
                        let focus = receiver.read(cx).focus.clone();
                        focus.focus(window, cx);
                        *input.borrow_mut() = Some(receiver.clone());
                        cx.new(|_| Root {
                            image: factory.image.clone(),
                            count: factory.count.clone(),
                            host_input: Some(receiver.into()),
                            observation: factory.observation.clone(),
                            button_shift: px(0.),
                        })
                    },
                )
                .map_err(anyhow::Error::from);
            *opened.borrow_mut() = Some(result);
        }
    });
    let outcome = run_loop(
        &host,
        &platform,
        &application,
        &factory,
        &opened,
        &input,
        smoke,
    );
    if let Err(error) = outcome {
        eprintln!("External Win32 host failed: {error:#}");
        std::process::exit(1);
    }
    Ok(())
}

fn run_loop(
    host: &HostWindow,
    platform: &WindowsPlatform,
    application: &ApplicationHandle,
    factory: &Factory,
    opened: &RefCell<Option<Result<WindowHandle<Root>>>>,
    input: &RefCell<Option<Entity<HostInput>>>,
    smoke: bool,
) -> Result<()> {
    let mut observation = LoopObservation::default();
    let deferred = factory.delivery_mode == PlatformFrameDeliveryMode::Deferred;
    let mut smoke = smoke.then(|| {
        if deferred {
            HostSmoke::Deferred(deferred_smoke::Smoke::new())
        } else {
            HostSmoke::Synchronous(smoke::Smoke::new())
        }
    });
    let mut window = None;
    let mut shutdown_started = None;
    let mut post_shutdown = false;
    let mut initial_completed = false;
    let launched_at = Instant::now();
    host.post_private(1)?;
    host.start_timer()?;
    loop {
        if let Some(result) = opened.borrow_mut().take() {
            window = Some(result.context("attaching GPUI to the application HWND")?);
        }
        ensure!(
            window.is_some() || launched_at.elapsed() < Duration::from_secs(15),
            "GPUI launch timed out"
        );
        if !initial_completed && let Some(window) = window.as_ref() {
            let facts = application
                .update(|cx| window.update(cx, |_, window, _| window.presentation_facts()))?;
            ensure!(
                facts.initial_presentation != WindowInitialPresentationStatus::Rejected,
                "the native initial presentation was rejected"
            );
            initial_completed =
                facts.initial_presentation == WindowInitialPresentationStatus::Completed;
            ensure!(
                initial_completed || launched_at.elapsed() < Duration::from_secs(15),
                "initial presentation timed out"
            );
            if initial_completed && smoke.is_none() {
                unsafe { KillTimer(Some(host.hwnd.get()), HOST_TIMER) }?;
            }
        }
        ensure!(
            factory.gpu.errors.load(Ordering::Relaxed) == 0
                && factory.observation.failures.get() == 0,
            "the shared GPU/UI renderer reported an error"
        );
        if deferred {
            let allow_submit = shutdown_started.is_none()
                && !host.close_requested.get()
                && smoke.as_ref().is_none_or(HostSmoke::auto_drain);
            factory.drain_pending(allow_submit)?;
        }
        if let Some(smoke) = smoke.as_mut() {
            smoke.advance(
                host,
                application,
                factory,
                window.as_ref(),
                input,
                &observation,
            )?;
        }
        let binding_closed = window.is_some() && application.update(|cx| cx.windows().is_empty());
        let should_shutdown = host.close_requested.get()
            || binding_closed
            || smoke
                .as_ref()
                .is_some_and(|smoke| smoke.ready_to_shutdown());
        if should_shutdown && shutdown_started.is_none() {
            host.start_timer()?;
            if smoke.is_some() {
                unsafe {
                    PostThreadMessageW(
                        GetCurrentThreadId(),
                        WM_QUIT,
                        WPARAM(HOST_QUIT_PROBE),
                        LPARAM(0),
                    )
                }?;
            }
            shutdown_started = Some(Instant::now());
            application.update(|cx| cx.quit());
        }
        if let Some(started) = shutdown_started {
            ensure!(
                started.elapsed() < Duration::from_secs(15),
                "GPUI shutdown/detachment timed out"
            );
            if platform.is_shutdown_complete() && !post_shutdown {
                host.verify()?;
                if let Some(smoke) = smoke.as_ref() {
                    smoke.verify_detached(factory)?;
                }
                host.post_private(2)?;
                post_shutdown = true;
            }
        }
        if post_shutdown
            && !host.destroyed.get()
            && host.last_private.get() == 2
            && (smoke.is_none() || observation.quit_probe_seen)
        {
            host.verify()?;
            ensure!(
                host.private_messages.get() == 2,
                "the host lost or duplicated a private message"
            );
            host.destroy()?;
        }
        let mut message = MSG::default();
        let pending = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) }.as_bool();
        let waiting = Instant::now();
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if !pending {
            observation.blocked += waiting.elapsed();
        }
        ensure!(
            status.0 != -1,
            "GetMessageW failed: {}",
            windows::core::Error::from_thread()
        );
        observation.messages += 1;
        if status.0 == 0 {
            ensure!(
                !platform.pretranslate_message(&message),
                "GPUI consumed a host WM_QUIT"
            );
            if message.wParam.0 == HOST_QUIT_PROBE
                && shutdown_started.is_some()
                && !host.destroyed.get()
            {
                ensure!(!observation.quit_probe_seen, "duplicate host quit probe");
                observation.quit_probe_seen = true;
                continue;
            }
            ensure!(
                host.destroyed.get() && host.ending_loop.get() && message.wParam.0 == 0,
                "GPUI posted an unexpected WM_QUIT before the host ended its loop"
            );
            ensure!(
                platform.is_shutdown_complete(),
                "the host loop ended before GPUI detached"
            );
            ensure!(
                !unsafe { PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_NOREMOVE) }
                    .as_bool(),
                "an unexpected extra WM_QUIT remained in the host queue"
            );
            break;
        }
        if !platform.pretranslate_message(&message) {
            // SAFETY: No App borrow is active while Win32 callbacks enter GPUI.
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    ensure!(
        factory.gpu.errors.load(Ordering::Relaxed) == 0 && factory.observation.failures.get() == 0,
        "the GPU/UI renderer reported an error during shutdown"
    );
    if let Some(smoke) = smoke.as_ref() {
        smoke.report(factory, &observation);
    }
    Ok(())
}
