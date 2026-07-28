use std::{
    cell::{Cell, RefCell},
    ffi::OsStr,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use ::open_gpui_util::{ResultExt, paths::SanitizedPath};
use anyhow::{Context as _, Result, anyhow};
use futures::channel::oneshot::{self, Receiver};
use itertools::Itertools;
use parking_lot::RwLock;
use smallvec::SmallVec;
use windows::{
    UI::ViewManagement::UISettings,
    Win32::{
        Foundation::*,
        Graphics::{Direct3D11::ID3D11Device, Gdi::*},
        Security::Credentials::*,
        System::{Com::*, LibraryLoader::*, Ole::*, Power::*, SystemInformation::*},
        UI::{Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::*,
};

use crate::*;
use open_gpui::*;

pub(crate) type RegisteredWindows = RwLock<SmallVec<[RegisteredWindow; 4]>>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct RegisteredWindow {
    hwnd: SafeHwnd,
    generation: usize,
    window_id: WindowId,
}

impl RegisteredWindow {
    pub(crate) fn new(hwnd: HWND, generation: usize, window_id: WindowId) -> Self {
        Self {
            hwnd: hwnd.into(),
            generation,
            window_id,
        }
    }

    pub(crate) fn as_raw(self) -> HWND {
        self.hwnd.as_raw()
    }

    pub(crate) fn generation(self) -> usize {
        self.generation
    }

    fn window_id(self) -> WindowId {
        self.window_id
    }

    pub(crate) fn matches(self, other: Self) -> bool {
        self.hwnd.as_raw() == other.hwnd.as_raw()
            && self.generation == other.generation
            && self.window_id == other.window_id
    }
}

static NEXT_NATIVE_WINDOW_GENERATION: AtomicUsize = AtomicUsize::new(1);

fn next_native_window_generation() -> usize {
    let generation = NEXT_NATIVE_WINDOW_GENERATION.fetch_add(1, Ordering::Relaxed);
    assert_ne!(
        generation, 0,
        "native window generation exhausted process-wide uniqueness"
    );
    generation
}

pub struct WindowsPlatform {
    inner: Rc<WindowsPlatformInner>,
    raw_window_handles: Arc<RegisteredWindows>,
    recovered_directx_devices: Arc<RwLock<Option<DirectXDevices>>>,
    vsync_owner_live: Arc<RwLock<bool>>,
    // The below members will never change throughout the entire lifecycle of the app.
    headless: bool,
    icon: HICON,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    direct_write_text_system: Option<Arc<DirectWriteTextSystem>>,
    drop_target_helper: Option<IDropTargetHelper>,
    /// Flag to instruct the `VSyncProvider` thread to invalidate the directx devices
    /// as resizing them has failed, causing us to have lost at least the render target.
    invalidate_devices: Arc<AtomicBool>,
    handle: HWND,
    suspend_resume_notification: RefCell<Option<HPOWERNOTIFY>>,
    disable_direct_composition: bool,
    #[cfg(test)]
    lifecycle_test_probe: Rc<NativeWindowLifecycleTestProbe>,
}

struct WindowsPlatformInner {
    state: WindowsPlatformState,
    recovered_directx_devices: Arc<RwLock<Option<DirectXDevices>>>,
    // The below members will never change throughout the entire lifecycle of the app.
    validation_number: usize,
    main_receiver: PriorityQueueReceiver<RunnableVariant>,
    dispatcher: Arc<WindowsDispatcher>,
}

pub(crate) struct WindowsPlatformState {
    callbacks: PlatformCallbacks,
    menus: RefCell<Vec<OwnedMenu>>,
    jump_list: RefCell<JumpList>,
    /// Shared with each window so `WM_SETCURSOR` can read it directly.
    pub(crate) cursor_visible: Arc<AtomicBool>,
    directx_devices: RefCell<Option<DirectXDevices>>,
}

struct WindowsPlatformConstructionGuard {
    hwnd: Option<HWND>,
    ole_initialized: bool,
}

impl WindowsPlatformConstructionGuard {
    fn initialize() -> Result<Self> {
        unsafe {
            OleInitialize(None).context("unable to initialize Windows OLE")?;
        }
        Ok(Self {
            hwnd: None,
            ole_initialized: true,
        })
    }

    fn own_hwnd(&mut self, hwnd: HWND) {
        self.hwnd = Some(hwnd);
    }

    fn commit(mut self) {
        self.hwnd = None;
        self.ole_initialized = false;
    }
}

impl Drop for WindowsPlatformConstructionGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(hwnd) = self.hwnd.take()
                && IsWindow(Some(hwnd)).as_bool()
            {
                DestroyWindow(hwnd)
                    .context("rolling back partially constructed platform window")
                    .log_err();
            }
            if self.ole_initialized {
                OleUninitialize();
            }
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct NativeWindowLifecycleTestProbe {
    fail_after_drag_drop_registration: Cell<bool>,
    fail_next_destroy: Cell<bool>,
    fail_next_initial_presentation: Cell<bool>,
    last_created_hwnd: Cell<Option<HWND>>,
    hidden_before_map: Cell<Option<bool>>,
    initial_presentation_hook: RefCell<Option<Box<dyn FnOnce(HWND)>>>,
}

#[cfg(test)]
impl NativeWindowLifecycleTestProbe {
    pub(crate) fn fail_next_after_drag_drop_registration(&self) {
        self.fail_after_drag_drop_registration.set(true);
    }

    pub(crate) fn take_fail_after_drag_drop_registration(&self) -> bool {
        self.fail_after_drag_drop_registration.replace(false)
    }

    pub(crate) fn fail_next_destroy(&self) {
        self.fail_next_destroy.set(true);
    }

    pub(crate) fn take_fail_next_destroy(&self) -> bool {
        self.fail_next_destroy.replace(false)
    }

    pub(crate) fn fail_next_initial_presentation(&self) {
        self.fail_next_initial_presentation.set(true);
    }

    pub(crate) fn take_fail_next_initial_presentation(&self) -> bool {
        self.fail_next_initial_presentation.replace(false)
    }

    pub(crate) fn install_initial_presentation_hook(&self, hook: impl FnOnce(HWND) + 'static) {
        let mut installed = self.initial_presentation_hook.borrow_mut();
        assert!(
            installed.is_none(),
            "initial-presentation test hook is already installed"
        );
        *installed = Some(Box::new(hook));
    }

    pub(crate) fn run_initial_presentation_hook(&self, hwnd: HWND) {
        let hook = self.initial_presentation_hook.borrow_mut().take();
        if let Some(hook) = hook {
            hook(hwnd);
        }
    }

    pub(crate) fn record_created_hwnd(&self, hwnd: HWND) {
        self.last_created_hwnd.set(Some(hwnd));
    }

    pub(crate) fn last_created_hwnd(&self) -> Option<HWND> {
        self.last_created_hwnd.get()
    }

    pub(crate) fn record_hidden_before_map(&self, hidden: bool) {
        self.hidden_before_map.set(Some(hidden));
    }

    pub(crate) fn hidden_before_map(&self) -> Option<bool> {
        self.hidden_before_map.get()
    }
}

#[derive(Default)]
struct PlatformCallbacks {
    open_urls: Cell<Option<Box<dyn FnMut(Vec<String>)>>>,
    quit: Cell<Option<Box<dyn FnMut()>>>,
    reopen: Cell<Option<Box<dyn FnMut()>>>,
    app_menu_action: Cell<Option<Box<dyn FnMut(&dyn Action)>>>,
    will_open_app_menu: Cell<Option<Box<dyn FnMut()>>>,
    validate_app_menu_command: Cell<Option<Box<dyn FnMut(&dyn Action) -> bool>>>,
    keyboard_layout_change: Cell<Option<Box<dyn FnMut()>>>,
    system_wake: Cell<Option<Box<dyn FnMut()>>>,
}

impl WindowsPlatformState {
    fn new(directx_devices: Option<DirectXDevices>) -> Self {
        let callbacks = PlatformCallbacks::default();
        let jump_list = JumpList::new();
        Self {
            callbacks,
            jump_list: RefCell::new(jump_list),
            cursor_visible: Arc::new(AtomicBool::new(true)),
            directx_devices: RefCell::new(directx_devices),
            menus: RefCell::new(Vec::new()),
        }
    }
}

impl WindowsPlatform {
    pub fn new(headless: bool) -> Result<Self> {
        let mut construction_guard = WindowsPlatformConstructionGuard::initialize()?;
        let (directx_devices, text_system, direct_write_text_system) = if !headless {
            let devices = DirectXDevices::new().context("Creating DirectX devices")?;
            let dw_text_system = Arc::new(
                DirectWriteTextSystem::new(&devices)
                    .context("Error creating DirectWriteTextSystem")?,
            );
            (
                Some(devices),
                dw_text_system.clone() as Arc<dyn PlatformTextSystem>,
                Some(dw_text_system),
            )
        } else {
            (
                None,
                Arc::new(open_gpui::NoopTextSystem::new()) as Arc<dyn PlatformTextSystem>,
                None,
            )
        };

        let (main_sender, main_receiver) = PriorityQueueReceiver::new();
        let validation_number = if usize::BITS == 64 {
            rand::random::<u64>() as usize
        } else {
            rand::random::<u32>() as usize
        };
        let raw_window_handles = Arc::new(RwLock::new(SmallVec::new()));
        let recovered_directx_devices = Arc::new(RwLock::new(None));
        let vsync_owner_live = Arc::new(RwLock::new(true));

        register_platform_window_class();
        let mut context = PlatformWindowCreateContext {
            inner: None,
            validation_number,
            main_sender: Some(main_sender),
            main_receiver: Some(main_receiver),
            directx_devices,
            recovered_directx_devices: recovered_directx_devices.clone(),
            dispatcher: None,
        };
        let result = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PLATFORM_WINDOW_CLASS_NAME,
                None,
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                Some(&raw const context as *const _),
            )
        };
        let handle = match result {
            Ok(handle) => handle,
            Err(create_error) => {
                if let Some(Err(inner_error)) = context.inner.take() {
                    return Err(inner_error);
                }
                return Err(create_error.into());
            }
        };
        construction_guard.own_hwnd(handle);
        let inner = context
            .inner
            .take()
            .context("CreateWindowExW did not initialize the platform window")??;
        let dispatcher = context
            .dispatcher
            .take()
            .context("CreateWindowExW did not run correctly")?;

        let disable_direct_composition = std::env::var(DISABLE_DIRECT_COMPOSITION)
            .is_ok_and(|value| value == "true" || value == "1");
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher);

        let drop_target_helper: Option<IDropTargetHelper> = if !headless {
            Some(unsafe {
                CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER)
                    .context("Error creating drop target helper.")?
            })
        } else {
            None
        };
        let icon = if !headless {
            load_icon().unwrap_or_default()
        } else {
            HICON::default()
        };

        let platform = Self {
            inner,
            handle,
            raw_window_handles,
            recovered_directx_devices,
            vsync_owner_live,
            headless,
            icon,
            background_executor,
            foreground_executor,
            text_system,
            direct_write_text_system,
            suspend_resume_notification: RefCell::new(None),
            disable_direct_composition,
            drop_target_helper,
            invalidate_devices: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            lifecycle_test_probe: Rc::default(),
        };
        construction_guard.commit();
        Ok(platform)
    }

    pub(crate) fn window_from_hwnd(&self, hwnd: HWND) -> Option<Rc<WindowsWindowInner>> {
        let registered = self
            .raw_window_handles
            .read()
            .iter()
            .find(|entry| entry.as_raw() == hwnd)
            .copied()?;
        let window = window_from_hwnd(hwnd)?;
        window.registration.matches(registered).then_some(window)
    }

    fn native_owner_for(&self, owner: AnyWindowHandle) -> Result<HWND> {
        let registered = {
            let registered_windows = self.raw_window_handles.read();
            registered_windows
                .iter()
                .find(|entry| entry.window_id() == owner.window_id())
                .copied()
        }
        .context("transient owner is not a live Windows platform window")?;
        let hwnd = registered.as_raw();
        anyhow::ensure!(
            unsafe { IsWindow(Some(hwnd)).as_bool() },
            "transient owner HWND is no longer live"
        );
        let window =
            window_from_hwnd(hwnd).context("transient owner HWND has no live GPUI window state")?;
        anyhow::ensure!(
            window.registration.matches(registered)
                && window.handle.window_id() == owner.window_id(),
            "transient owner no longer matches its registered window generation"
        );
        Ok(hwnd)
    }

    fn generate_creation_info(&self) -> WindowCreationInfo {
        WindowCreationInfo {
            icon: self.icon,
            executor: self.foreground_executor.clone(),
            current_cursor: load_cursor(CursorStyle::Arrow),
            cursor_visible: self.inner.state.cursor_visible.clone(),
            drop_target_helper: self.drop_target_helper.clone().unwrap(),
            validation_number: self.inner.validation_number,
            native_window_generation: next_native_window_generation(),
            main_receiver: self.inner.main_receiver.clone(),
            platform_window_handle: self.handle,
            raw_window_handles: Arc::downgrade(&self.raw_window_handles),
            recovered_directx_devices: self.recovered_directx_devices.clone(),
            disable_direct_composition: self.disable_direct_composition,
            directx_devices: self.inner.state.directx_devices.borrow().clone().unwrap(),
            invalidate_devices: self.invalidate_devices.clone(),
            #[cfg(test)]
            lifecycle_test_probe: self.lifecycle_test_probe.clone(),
        }
    }

    fn set_dock_menus(&self, menus: Vec<MenuItem>) {
        let mut actions = Vec::new();
        menus.into_iter().for_each(|menu| {
            if let Some(dock_menu) = DockMenuItem::new(menu).log_err() {
                actions.push(dock_menu);
            }
        });
        self.inner.state.jump_list.borrow_mut().dock_menus = actions;
        let borrow = self.inner.state.jump_list.borrow();
        let dock_menus = borrow
            .dock_menus
            .iter()
            .map(|menu| (menu.name.clone(), menu.description.clone()))
            .collect::<Vec<_>>();
        let recent_workspaces = borrow.recent_workspaces.clone();
        self.background_executor
            .spawn(async move {
                update_jump_list(&recent_workspaces, &dock_menus).log_err();
            })
            .detach();
    }

    fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        let mut actions = Vec::new();
        menus.into_iter().for_each(|menu| {
            if let Some(dock_menu) = DockMenuItem::new(menu).log_err() {
                actions.push(dock_menu);
            }
        });
        let mut jump_list = self.inner.state.jump_list.borrow_mut();
        jump_list.dock_menus = actions;
        jump_list.recent_workspaces = entries.into();
        let dock_menus = jump_list
            .dock_menus
            .iter()
            .map(|menu| (menu.name.clone(), menu.description.clone()))
            .collect::<Vec<_>>();
        let recent_workspaces = jump_list.recent_workspaces.clone();
        self.background_executor.spawn(async move {
            update_jump_list(&recent_workspaces, &dock_menus)
                .log_err()
                .unwrap_or_default()
        })
    }

    fn find_current_foreground_window(&self) -> Option<HWND> {
        let foreground_window_hwnd = unsafe { GetForegroundWindow() };
        if foreground_window_hwnd.is_invalid() {
            return None;
        }
        self.raw_window_handles
            .read()
            .iter()
            .find(|hwnd| hwnd.as_raw() == foreground_window_hwnd)
            .map(|hwnd| hwnd.as_raw())
    }

    fn begin_vsync_thread(&self) {
        let Some(directx_devices) = self.inner.state.directx_devices.borrow().clone() else {
            return;
        };
        let Some(direct_write_text_system) = &self.direct_write_text_system else {
            return;
        };
        let mut directx_device = directx_devices;
        let platform_window: SafeHwnd = self.handle.into();
        let validation_number = self.inner.validation_number;
        let all_windows = Arc::downgrade(&self.raw_window_handles);
        let text_system = Arc::downgrade(direct_write_text_system);
        let invalidate_devices = self.invalidate_devices.clone();
        let recovered_directx_devices = self.recovered_directx_devices.clone();
        let vsync_owner_live = self.vsync_owner_live.clone();

        std::thread::Builder::new()
            .name("VSyncProvider".to_owned())
            .spawn(move || {
                let vsync_provider = VSyncProvider::new();
                loop {
                    vsync_provider.wait_for_vsync();
                    if !*vsync_owner_live.read() {
                        break;
                    }
                    if check_device_lost(&directx_device.device)
                        || invalidate_devices.fetch_and(false, Ordering::Acquire)
                    {
                        if let Err(err) = handle_gpu_device_lost(
                            &mut directx_device,
                            platform_window.as_raw(),
                            validation_number,
                            &all_windows,
                            &text_system,
                            &recovered_directx_devices,
                            &vsync_owner_live,
                        ) {
                            log::error!(
                                "Failed to recover DirectX device after device lost: {err:?}"
                            );
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    }
                    let Some(all_windows) = all_windows.upgrade() else {
                        break;
                    };
                    let registered_windows = all_windows.read().clone();
                    dispatch_registered_window_snapshot(
                        &all_windows,
                        registered_windows,
                        |registered_window| {
                            with_live_vsync_owner(&vsync_owner_live, || unsafe {
                                let _ = RedrawWindow(
                                    Some(registered_window.as_raw()),
                                    None,
                                    None,
                                    RDW_INVALIDATE,
                                );
                            });
                        },
                    );
                }
            })
            .unwrap();
    }
}

fn translate_accelerator(msg: &MSG) -> Option<()> {
    if msg.message != WM_KEYDOWN && msg.message != WM_SYSKEYDOWN {
        return None;
    }

    let result = unsafe {
        SendMessageW(
            msg.hwnd,
            WM_GPUI_KEYDOWN,
            Some(msg.wParam),
            Some(msg.lParam),
        )
    };
    (result.0 == 0).then_some(())
}

fn windows_window_capabilities() -> PlatformWindowCapabilities {
    PlatformWindowCapabilities {
        creation: PlatformWindowCreationCapabilities {
            focus_on_appearing: WindowCreationSupport::Supported,
            transient_for: WindowCreationSupport::Supported,
            initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
        },
        mutations: PlatformWindowMutationCapabilities {
            position: WindowMutationSupport::CreationOnly,
            size: WindowMutationSupport::Live,
            windowed: WindowMutationSupport::Live,
            maximized: WindowMutationSupport::Live,
            fullscreen: WindowMutationSupport::Live,
            minimized: WindowMutationSupport::Unsupported,
            restore_bounds: WindowMutationSupport::CreationOnly,
            pointer_input: WindowMutationSupport::Live,
            activation_policy: WindowMutationSupport::Live,
            alpha: WindowMutationSupport::CreationOnly,
            topmost: WindowMutationSupport::Unsupported,
            taskbar_visibility: WindowMutationSupport::Unsupported,
            coordinate_space: WindowCoordinateSpace::WindowLocal,
        },
    }
}

impl Platform for WindowsPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(
            WindowsKeyboardLayout::new()
                .log_err()
                .unwrap_or(WindowsKeyboardLayout::unknown()),
        )
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(WindowsKeyboardMapper::new())
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.inner
            .state
            .callbacks
            .keyboard_layout_change
            .set(Some(callback));
    }

    fn on_thermal_state_change(&self, _callback: Box<dyn FnMut()>) {}

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        on_finish_launching();
        if !self.headless {
            self.begin_vsync_thread();
        }

        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if translate_accelerator(&msg).is_none() {
                    _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        self.inner
            .with_callback(|callbacks| &callbacks.quit, |callback| callback());

        // Bypass the CRT exit logic, which runs atexit handlers before calling ExitProcess.
        // aws-lc registers an atexit handler that intentionally acquires a lock without releasing it.
        // aws-lc also has thread_local objects which acquire this lock in their destructor.
        // Destructors for thread_locals run under the loader lock, so there is a race condition
        // where, if a thread exits after atexit handlers have run, the TLS destructors will block
        // indefinitely on this lock while holding the loader lock. Since ExitProcess also requires
        // the loader lock, process teardown will deadlock.
        unsafe {
            windows::Win32::System::Threading::ExitProcess(0);
        }
    }

    fn quit(&self) {
        self.foreground_executor()
            .spawn(async { unsafe { PostQuitMessage(0) } })
            .detach();
    }

    fn restart(&self, binary_path: Option<PathBuf>) {
        let pid = std::process::id();
        let Some(app_path) = binary_path.or(self.app_path().log_err()) else {
            return;
        };
        let script = format!(
            r#"
            $pidToWaitFor = {}
            $exePath = "{}"

            while ($true) {{
                $process = Get-Process -Id $pidToWaitFor -ErrorAction SilentlyContinue
                if (-not $process) {{
                    Start-Process -FilePath $exePath
                    break
                }}
                Start-Sleep -Seconds 0.1
            }}
            "#,
            pid,
            app_path.display(),
        );

        // Defer spawning to the foreground executor so it runs after the
        // current `AppCell` borrow is released. On Windows, `Command::spawn()`
        // can pump the Win32 message loop (via `CreateProcessW`), which
        // re-enters message handling possibly resulting in another mutable
        // borrow of the `AppCell` ending up with a double borrow panic
        self.foreground_executor
            .spawn(async move {
                #[allow(
                    clippy::disallowed_methods,
                    reason = "We are restarting ourselves, using std command thus is fine"
                )]
                let restart_process = ::open_gpui_util::command::new_std_command(
                    ::open_gpui_util::shell::get_windows_system_shell(),
                )
                .arg("-command")
                .arg(script)
                .spawn();

                match restart_process {
                    Ok(_) => unsafe { PostQuitMessage(0) },
                    Err(e) => log::error!("failed to spawn restart script: {:?}", e),
                }
            })
            .detach();
    }

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {}

    fn hide_other_apps(&self) {
        log::debug!("WindowsPlatform::hide_other_apps is not supported on Windows");
    }

    fn unhide_other_apps(&self) {
        log::debug!("WindowsPlatform::unhide_other_apps is not supported on Windows");
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        WindowsDisplay::displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        WindowsDisplay::primary_monitor().map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
    }

    #[cfg(feature = "screen-capture")]
    fn is_screen_capture_supported(&self) -> bool {
        true
    }

    #[cfg(feature = "screen-capture")]
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        open_gpui::scap_screen_capture::scap_screen_sources(&self.foreground_executor)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        let foreground_window_hwnd = unsafe { GetForegroundWindow() };
        self.window_from_hwnd(foreground_window_hwnd)
            .map(|inner| inner.handle)
    }

    fn focused_window(&self) -> PlatformFocusedWindow {
        PlatformFocusedWindow::from_window(self.active_window())
    }

    fn hovered_window(&self) -> PlatformHoveredWindow {
        let mut cursor_position = POINT::default();
        if unsafe { GetCursorPos(&mut cursor_position) }.is_err() {
            return PlatformHoveredWindow::Unavailable;
        }
        let hovered_window_hwnd = unsafe { WindowFromPoint(cursor_position) };
        PlatformHoveredWindow::from_window(
            self.window_from_hwnd(hovered_window_hwnd)
                .map(|inner| inner.handle),
        )
    }

    fn viewport_capabilities(&self) -> PlatformViewportCapabilities {
        PlatformViewportCapabilities {
            platform_viewport_windows: true,
            global_window_bounds: false,
            display_work_area: true,
            dpi_scale: true,
            hovered_window_ignores_no_input: true,
            ..Default::default()
        }
    }

    fn window_capabilities(
        &self,
        _kind: &WindowKind,
        _display_id: Option<DisplayId>,
    ) -> PlatformWindowCapabilities {
        windows_window_capabilities()
    }

    fn mouse_button_is_pressed(&self, button: MouseButton) -> Option<bool> {
        let virtual_key = match button {
            MouseButton::Left => VK_LBUTTON,
            MouseButton::Right => VK_RBUTTON,
            MouseButton::Middle => VK_MBUTTON,
            MouseButton::Navigate(NavigationDirection::Back) => VK_XBUTTON1,
            MouseButton::Navigate(NavigationDirection::Forward) => VK_XBUTTON2,
        };
        Some((unsafe { GetAsyncKeyState(i32::from(virtual_key.0)) } as u16 & 0x8000) != 0)
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        let transient_owner_hwnd = options
            .transient_for
            .map(|owner| self.native_owner_for(owner))
            .transpose()?;
        let window = WindowsWindow::new(
            handle,
            options,
            transient_owner_hwnd,
            self.generate_creation_info(),
        )?;
        self.raw_window_handles.write().push(window.0.registration);

        Ok(Box::new(window))
    }

    fn window_appearance(&self) -> WindowAppearance {
        system_appearance().log_err().unwrap_or_default()
    }

    fn open_url(&self, url: &str) {
        if url.is_empty() {
            return;
        }
        let url_string = url.to_string();
        self.background_executor()
            .spawn(async move {
                open_target(&url_string)
                    .with_context(|| format!("Opening url: {}", url_string))
                    .log_err();
            })
            .detach();
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.inner.state.callbacks.open_urls.set(Some(callback));
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        let window = self.find_current_foreground_window();
        self.foreground_executor()
            .spawn(async move {
                let _ = tx.send(file_open_dialog(options, window));
            })
            .detach();

        rx
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> Receiver<Result<Option<PathBuf>>> {
        let directory = directory.to_owned();
        let suggested_name = suggested_name.map(|s| s.to_owned());
        let (tx, rx) = oneshot::channel();
        let window = self.find_current_foreground_window();
        self.foreground_executor()
            .spawn(async move {
                let _ = tx.send(file_save_dialog(directory, suggested_name, window));
            })
            .detach();

        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        // The FOS_PICKFOLDERS flag toggles between "only files" and "only folders".
        false
    }

    fn reveal_path(&self, path: &Path) {
        if path.as_os_str().is_empty() {
            return;
        }
        let path = path.to_path_buf();
        self.background_executor()
            .spawn(async move {
                open_target_in_explorer(&path)
                    .with_context(|| format!("Revealing path {} in explorer", path.display()))
                    .log_err();
            })
            .detach();
    }

    fn open_with_system(&self, path: &Path) {
        if path.as_os_str().is_empty() {
            return;
        }
        let path = path.to_path_buf();
        self.background_executor()
            .spawn(async move {
                open_target(&path)
                    .with_context(|| format!("Opening {} with system", path.display()))
                    .log_err();
            })
            .detach();
    }

    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        self.inner.state.callbacks.quit.set(Some(callback));
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.inner.state.callbacks.reopen.set(Some(callback));
    }

    fn on_system_wake(&self, callback: Box<dyn FnMut()>) {
        self.inner.state.callbacks.system_wake.set(Some(callback));

        let mut notification = self.suspend_resume_notification.borrow_mut();
        if notification.is_none() {
            *notification = unsafe {
                // SAFETY: self.handle is the platform window that receives WM_POWERBROADCAST.
                RegisterSuspendResumeNotification(
                    HANDLE(self.handle.0),
                    DEVICE_NOTIFY_WINDOW_HANDLE,
                )
                .log_err()
            };
        }
    }

    fn set_menus(&self, menus: Vec<Menu>, _keymap: &Keymap) {
        *self.inner.state.menus.borrow_mut() = menus.into_iter().map(|menu| menu.owned()).collect();
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        Some(self.inner.state.menus.borrow().clone())
    }

    fn set_dock_menu(&self, menus: Vec<MenuItem>, _keymap: &Keymap) {
        self.set_dock_menus(menus);
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.inner
            .state
            .callbacks
            .app_menu_action
            .set(Some(callback));
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.inner
            .state
            .callbacks
            .will_open_app_menu
            .set(Some(callback));
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.inner
            .state
            .callbacks
            .validate_app_menu_command
            .set(Some(callback));
    }

    fn app_path(&self) -> Result<PathBuf> {
        Ok(std::env::current_exe()?)
    }

    // todo(windows)
    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        anyhow::bail!("not yet implemented");
    }

    fn hide_cursor_until_mouse_moves(&self) {
        if !self
            .inner
            .state
            .cursor_visible
            .swap(false, Ordering::Relaxed)
        {
            return;
        }

        for handle in self.raw_window_handles.read().iter() {
            let Some(window) = window_from_hwnd(handle.as_raw()) else {
                continue;
            };
            if window.state.hovered.get() {
                unsafe { SetCursor(None) };
                break;
            }
        }
    }

    fn is_cursor_visible(&self) -> bool {
        self.inner.state.cursor_visible.load(Ordering::Relaxed)
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        should_auto_hide_scrollbars().log_err().unwrap_or(false)
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        write_to_clipboard(item);
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        read_from_clipboard()
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        if let Err(err) = validate_credential_blob_size(password.len()) {
            return Task::ready(Err(err));
        }

        let password = password.to_vec();
        let mut username = username.encode_utf16().chain(Some(0)).collect_vec();
        let mut target_name = windows_credentials_target_name(url)
            .encode_utf16()
            .chain(Some(0))
            .collect_vec();
        self.foreground_executor().spawn(async move {
            let credentials = CREDENTIALW {
                LastWritten: unsafe { GetSystemTimeAsFileTime() },
                Flags: CRED_FLAGS(0),
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR::from_raw(target_name.as_mut_ptr()),
                CredentialBlobSize: password.len() as u32,
                CredentialBlob: password.as_ptr() as *mut _,
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                UserName: PWSTR::from_raw(username.as_mut_ptr()),
                ..CREDENTIALW::default()
            };
            unsafe {
                CredWriteW(&credentials, 0).map_err(|err| {
                    anyhow!(
                        "Failed to write credentials to Windows Credential Manager: {}",
                        err,
                    )
                })?;
            }
            Ok(())
        })
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        let target_name = windows_credentials_target_name(url)
            .encode_utf16()
            .chain(Some(0))
            .collect_vec();
        self.foreground_executor().spawn(async move {
            let mut credentials: *mut CREDENTIALW = std::ptr::null_mut();
            let result = unsafe {
                CredReadW(
                    PCWSTR::from_raw(target_name.as_ptr()),
                    CRED_TYPE_GENERIC,
                    None,
                    &mut credentials,
                )
            };

            if let Err(err) = result {
                // ERROR_NOT_FOUND means the credential doesn't exist.
                // Return Ok(None) to match macOS and Linux behavior.
                if err.code() == ERROR_NOT_FOUND.to_hresult() {
                    return Ok(None);
                }
                return Err(err.into());
            }

            if credentials.is_null() {
                Ok(None)
            } else {
                let username: String = unsafe { (*credentials).UserName.to_string()? };
                let credential_blob = unsafe {
                    std::slice::from_raw_parts(
                        (*credentials).CredentialBlob,
                        (*credentials).CredentialBlobSize as usize,
                    )
                };
                let password = credential_blob.to_vec();
                unsafe { CredFree(credentials as *const _ as _) };
                Ok(Some((username, password)))
            }
        })
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        let target_name = windows_credentials_target_name(url)
            .encode_utf16()
            .chain(Some(0))
            .collect_vec();
        self.foreground_executor().spawn(async move {
            unsafe {
                CredDeleteW(
                    PCWSTR::from_raw(target_name.as_ptr()),
                    CRED_TYPE_GENERIC,
                    None,
                )?
            };
            Ok(())
        })
    }

    fn register_url_scheme(&self, _: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Err(anyhow!("register_url_scheme unimplemented")))
    }

    fn perform_dock_menu_action(&self, action: usize) {
        unsafe {
            PostMessageW(
                Some(self.handle),
                WM_GPUI_DOCK_MENU_ACTION,
                WPARAM(self.inner.validation_number),
                LPARAM(action as isize),
            )
            .log_err();
        }
    }

    fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        self.update_jump_list(menus, entries)
    }
}

impl WindowsPlatformInner {
    fn new(context: &mut PlatformWindowCreateContext) -> Result<Rc<Self>> {
        let state = WindowsPlatformState::new(context.directx_devices.take());
        Ok(Rc::new(Self {
            state,
            recovered_directx_devices: context.recovered_directx_devices.clone(),
            dispatcher: context
                .dispatcher
                .as_ref()
                .context("missing dispatcher")?
                .clone(),
            validation_number: context.validation_number,
            main_receiver: context
                .main_receiver
                .take()
                .context("missing main receiver")?,
        }))
    }

    /// Calls `project` to project to the corresponding callback field, removes it from callbacks, calls `f` with the callback and then puts the callback back.
    fn with_callback<T>(
        &self,
        project: impl Fn(&PlatformCallbacks) -> &Cell<Option<T>>,
        f: impl FnOnce(&mut T),
    ) {
        let callback = project(&self.state.callbacks).take();
        if let Some(mut callback) = callback {
            f(&mut callback);
            project(&self.state.callbacks).set(Some(callback));
        }
    }

    fn handle_msg(
        self: &Rc<Self>,
        handle: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let handled = match msg {
            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD
            | WM_GPUI_DOCK_MENU_ACTION
            | WM_GPUI_KEYBOARD_LAYOUT_CHANGED
            | WM_GPUI_GPU_DEVICE_LOST => self.handle_gpui_events(msg, wparam, lparam),
            WM_POWERBROADCAST => self.handle_power_broadcast(wparam),
            _ => None,
        };
        if let Some(result) = handled {
            LRESULT(result)
        } else {
            unsafe { DefWindowProcW(handle, msg, wparam, lparam) }
        }
    }

    fn handle_gpui_events(&self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if wparam.0 != self.validation_number {
            log::error!("Wrong validation number while processing message: {message}");
            return None;
        }
        match message {
            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD => self.run_foreground_task(),
            WM_GPUI_DOCK_MENU_ACTION => self.handle_dock_action_event(lparam.0 as _),
            WM_GPUI_KEYBOARD_LAYOUT_CHANGED => self.handle_keyboard_layout_change(),
            WM_GPUI_GPU_DEVICE_LOST => self.handle_device_lost(),
            _ => unreachable!(),
        }
    }

    #[inline]
    fn run_foreground_task(&self) -> Option<isize> {
        const MAIN_TASK_TIMEOUT: u128 = 10;

        let start = std::time::Instant::now();
        'tasks: loop {
            'timeout_loop: loop {
                if start.elapsed().as_millis() >= MAIN_TASK_TIMEOUT {
                    log::debug!("foreground task timeout reached");
                    // we spent our budget on gpui tasks, we likely have a lot of work queued so drain system events first to stay responsive
                    // then quit out of foreground work to allow us to process other gpui events first before returning back to foreground task work
                    // if we don't we might not for example process window quit events
                    let mut msg = MSG::default();
                    let process_message = |msg: &_| {
                        if translate_accelerator(msg).is_none() {
                            _ = unsafe { TranslateMessage(msg) };
                            unsafe { DispatchMessageW(msg) };
                        }
                    };
                    let peek_msg = |msg: &mut _, msg_kind| unsafe {
                        PeekMessageW(msg, None, 0, 0, PM_REMOVE | msg_kind).as_bool()
                    };
                    // We need to process a paint message here as otherwise we will re-enter `run_foreground_task` before painting if we have work remaining.
                    // The reason for this is that windows prefers custom application message processing over system messages.
                    if peek_msg(&mut msg, PM_QS_PAINT) {
                        process_message(&msg);
                    }
                    while peek_msg(&mut msg, PM_QS_INPUT) {
                        process_message(&msg);
                    }
                    // Allow the main loop to process other gpui events before going back into `run_foreground_task`
                    unsafe {
                        if let Err(_) = PostMessageW(
                            Some(self.dispatcher.platform_window_handle.as_raw()),
                            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD,
                            WPARAM(self.validation_number),
                            LPARAM(0),
                        ) {
                            self.dispatcher.wake_posted.store(false, Ordering::Release);
                        };
                    }
                    break 'tasks;
                }
                let mut main_receiver = self.main_receiver.clone();
                match main_receiver.try_pop() {
                    Ok(Some(runnable)) => WindowsDispatcher::execute_runnable(runnable),
                    _ => break 'timeout_loop,
                }
            }

            // Someone could enqueue a Runnable here. The flag is still true, so they will not PostMessage.
            // We need to check for those Runnables after we clear the flag.
            self.dispatcher.wake_posted.store(false, Ordering::Release);
            let mut main_receiver = self.main_receiver.clone();
            match main_receiver.try_pop() {
                Ok(Some(runnable)) => {
                    self.dispatcher.wake_posted.store(true, Ordering::Release);

                    WindowsDispatcher::execute_runnable(runnable);
                }
                _ => break 'tasks,
            }
        }

        Some(0)
    }

    fn handle_dock_action_event(&self, action_idx: usize) -> Option<isize> {
        let Some(action) = self
            .state
            .jump_list
            .borrow()
            .dock_menus
            .get(action_idx)
            .map(|dock_menu| dock_menu.action.boxed_clone())
        else {
            log::error!("Dock menu for index {action_idx} not found");
            return Some(1);
        };
        self.with_callback(
            |callbacks| &callbacks.app_menu_action,
            |callback| callback(&*action),
        );
        Some(0)
    }

    fn handle_keyboard_layout_change(&self) -> Option<isize> {
        self.with_callback(
            |callbacks| &callbacks.keyboard_layout_change,
            |callback| callback(),
        );
        Some(0)
    }

    fn handle_power_broadcast(&self, wparam: WPARAM) -> Option<isize> {
        if wparam.0 as u32 == PBT_APMRESUMEAUTOMATIC {
            self.with_callback(|callbacks| &callbacks.system_wake, |callback| callback());
        }
        Some(1)
    }

    fn handle_device_lost(&self) -> Option<isize> {
        let directx_devices = self.recovered_directx_devices.read().clone()?;
        self.state.directx_devices.borrow_mut().take();
        *self.state.directx_devices.borrow_mut() = Some(directx_devices);

        Some(0)
    }
}

impl Drop for WindowsPlatform {
    fn drop(&mut self) {
        *self.vsync_owner_live.write() = false;
        let registered_windows = {
            let mut registered_windows = self.raw_window_handles.write();
            std::mem::take(&mut *registered_windows)
        };
        for registered_window in registered_windows {
            let hwnd = registered_window.as_raw();
            if let Some(window) = window_from_hwnd(hwnd)
                && window.registration.matches(registered_window)
            {
                window.destroy_native_window();
            } else if unsafe { IsWindow(Some(hwnd)).as_bool() } {
                unsafe {
                    RevokeDragDrop(hwnd).log_err();
                    DestroyWindow(hwnd)
                        .context("destroying orphaned native window during platform teardown")
                        .log_err();
                }
            }
        }

        unsafe {
            if let Some(notification) = self.suspend_resume_notification.borrow_mut().take() {
                // SAFETY: notification was returned by RegisterSuspendResumeNotification.
                UnregisterSuspendResumeNotification(notification).log_err();
            }
            DestroyWindow(self.handle)
                .context("Destroying platform window")
                .log_err();
            OleUninitialize();
        }
    }
}

pub(crate) struct WindowCreationInfo {
    pub(crate) icon: HICON,
    pub(crate) executor: ForegroundExecutor,
    pub(crate) current_cursor: Option<HCURSOR>,
    pub(crate) cursor_visible: Arc<AtomicBool>,
    pub(crate) drop_target_helper: IDropTargetHelper,
    pub(crate) validation_number: usize,
    pub(crate) native_window_generation: usize,
    pub(crate) main_receiver: PriorityQueueReceiver<RunnableVariant>,
    pub(crate) platform_window_handle: HWND,
    pub(crate) raw_window_handles: std::sync::Weak<RegisteredWindows>,
    pub(crate) recovered_directx_devices: Arc<RwLock<Option<DirectXDevices>>>,
    pub(crate) disable_direct_composition: bool,
    pub(crate) directx_devices: DirectXDevices,
    /// Flag to instruct the `VSyncProvider` thread to invalidate the directx devices
    /// as resizing them has failed, causing us to have lost at least the render target.
    pub(crate) invalidate_devices: Arc<AtomicBool>,
    #[cfg(test)]
    pub(crate) lifecycle_test_probe: Rc<NativeWindowLifecycleTestProbe>,
}

struct PlatformWindowCreateContext {
    inner: Option<Result<Rc<WindowsPlatformInner>>>,
    validation_number: usize,
    main_sender: Option<PriorityQueueSender<RunnableVariant>>,
    main_receiver: Option<PriorityQueueReceiver<RunnableVariant>>,
    directx_devices: Option<DirectXDevices>,
    recovered_directx_devices: Arc<RwLock<Option<DirectXDevices>>>,
    dispatcher: Option<Arc<WindowsDispatcher>>,
}

fn open_target(target: impl AsRef<OsStr>) -> Result<()> {
    let target = target.as_ref();
    let ret = unsafe {
        ShellExecuteW(
            None,
            windows::core::w!("open"),
            &HSTRING::from(target),
            None,
            None,
            SW_SHOWDEFAULT,
        )
    };
    if ret.0 as isize <= 32 {
        Err(anyhow::anyhow!(
            "Unable to open target: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

fn open_target_in_explorer(target: &Path) -> Result<()> {
    let dir = target.parent().context("No parent folder found")?;
    let desktop = unsafe { SHGetDesktopFolder()? };

    let mut dir_item = std::ptr::null_mut();
    unsafe {
        desktop.ParseDisplayName(
            HWND::default(),
            None,
            &HSTRING::from(dir),
            None,
            &mut dir_item,
            std::ptr::null_mut(),
        )?;
    }

    let mut file_item = std::ptr::null_mut();
    unsafe {
        desktop.ParseDisplayName(
            HWND::default(),
            None,
            &HSTRING::from(target),
            None,
            &mut file_item,
            std::ptr::null_mut(),
        )?;
    }

    let highlight = [file_item as *const _];
    unsafe { SHOpenFolderAndSelectItems(dir_item as _, Some(&highlight), 0) }.or_else(|err| {
        if err.code().0 == ERROR_FILE_NOT_FOUND.0 as i32 {
            // On some systems, the above call mysteriously fails with "file not
            // found" even though the file is there.  In these cases, ShellExecute()
            // seems to work as a fallback (although it won't select the file).
            open_target(dir).context("Opening target parent folder")
        } else {
            Err(anyhow::anyhow!("Can not open target path: {}", err))
        }
    })
}

fn file_open_dialog(
    options: PathPromptOptions,
    window: Option<HWND>,
) -> Result<Option<Vec<PathBuf>>> {
    let folder_dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)? };

    let mut dialog_options = FOS_FILEMUSTEXIST;
    if options.multiple {
        dialog_options |= FOS_ALLOWMULTISELECT;
    }
    if options.directories {
        dialog_options |= FOS_PICKFOLDERS;
    }

    unsafe {
        folder_dialog.SetOptions(dialog_options)?;

        if let Some(prompt) = options.prompt {
            let prompt: &str = &prompt;
            folder_dialog.SetOkButtonLabel(&HSTRING::from(prompt))?;
        }

        if folder_dialog.Show(window).is_err() {
            // User cancelled
            return Ok(None);
        }
    }

    let results = unsafe { folder_dialog.GetResults()? };
    let file_count = unsafe { results.GetCount()? };
    if file_count == 0 {
        return Ok(None);
    }

    let mut paths = Vec::with_capacity(file_count as usize);
    for i in 0..file_count {
        let item = unsafe { results.GetItemAt(i)? };
        let path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)?.to_string()? };
        paths.push(PathBuf::from(path));
    }

    Ok(Some(paths))
}

fn file_save_dialog(
    directory: PathBuf,
    suggested_name: Option<String>,
    window: Option<HWND>,
) -> Result<Option<PathBuf>> {
    let dialog: IFileSaveDialog = unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL)? };
    if !directory.to_string_lossy().is_empty()
        && let Some(full_path) = directory
            .canonicalize()
            .context("failed to canonicalize directory")
            .log_err()
    {
        let full_path = SanitizedPath::new(&full_path);
        let full_path_string = full_path.to_string();
        let path_item: IShellItem =
            unsafe { SHCreateItemFromParsingName(&HSTRING::from(full_path_string), None)? };
        unsafe {
            dialog
                .SetFolder(&path_item)
                .context("failed to set dialog folder")
                .log_err()
        };
    }

    if let Some(suggested_name) = suggested_name {
        unsafe {
            dialog
                .SetFileName(&HSTRING::from(suggested_name))
                .context("failed to set file name")
                .log_err()
        };
    }

    unsafe {
        dialog.SetFileTypes(&[Common::COMDLG_FILTERSPEC {
            pszName: windows::core::w!("All files"),
            pszSpec: windows::core::w!("*.*"),
        }])?;
        if dialog.Show(window).is_err() {
            // User cancelled
            return Ok(None);
        }
    }
    let shell_item = unsafe { dialog.GetResult()? };
    let file_path_string = unsafe {
        let pwstr = shell_item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let string = pwstr.to_string()?;
        CoTaskMemFree(Some(pwstr.0 as _));
        string
    };
    Ok(Some(PathBuf::from(file_path_string)))
}

fn load_icon() -> Result<HICON> {
    let module = unsafe { GetModuleHandleW(None).context("unable to get module handle")? };
    let handle = unsafe {
        LoadImageW(
            Some(module.into()),
            windows::core::PCWSTR(1 as _),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        )
        .context("unable to load icon file")?
    };
    Ok(HICON(handle.0))
}

#[inline]
fn should_auto_hide_scrollbars() -> Result<bool> {
    let ui_settings = UISettings::new()?;
    Ok(ui_settings.AutoHideScrollBars()?)
}

fn check_device_lost(device: &ID3D11Device) -> bool {
    let device_state = unsafe { device.GetDeviceRemovedReason() };
    match device_state {
        Ok(_) => false,
        Err(err) => {
            log::error!("DirectX device lost detected: {:?}", err);
            true
        }
    }
}

fn handle_gpu_device_lost(
    directx_devices: &mut DirectXDevices,
    platform_window: HWND,
    validation_number: usize,
    all_windows: &std::sync::Weak<RegisteredWindows>,
    text_system: &std::sync::Weak<DirectWriteTextSystem>,
    recovered_directx_devices: &RwLock<Option<DirectXDevices>>,
    vsync_owner_live: &RwLock<bool>,
) -> Result<()> {
    // Here we wait a bit to ensure the system has time to recover from the device lost state.
    // If we don't wait, the final drawing result will be blank.
    std::thread::sleep(std::time::Duration::from_millis(350));

    *directx_devices = try_to_recover_from_device_lost(|| {
        DirectXDevices::new().context("Failed to recreate new DirectX devices after device lost")
    })?;
    log::info!("DirectX devices successfully recreated.");
    *recovered_directx_devices.write() = Some(directx_devices.clone());

    with_live_vsync_owner(vsync_owner_live, || unsafe {
        PostMessageW(
            Some(platform_window),
            WM_GPUI_GPU_DEVICE_LOST,
            WPARAM(validation_number),
            LPARAM::default(),
        )
        .log_err();
    });

    if let Some(text_system) = text_system.upgrade() {
        text_system.handle_gpu_lost(&directx_devices)?;
    }
    if let Some(all_windows) = all_windows.upgrade() {
        let registered_windows = all_windows.read().clone();
        let registered_windows =
            dispatch_registered_window_snapshot(&all_windows, registered_windows, |window| {
                with_live_vsync_owner(vsync_owner_live, || unsafe {
                    PostMessageW(
                        Some(window.as_raw()),
                        WM_GPUI_GPU_DEVICE_LOST,
                        WPARAM(window.generation()),
                        LPARAM::default(),
                    )
                    .log_err();
                });
            });
        dispatch_registered_window_snapshot(&all_windows, registered_windows, |window| {
            with_live_vsync_owner(vsync_owner_live, || unsafe {
                PostMessageW(
                    Some(window.as_raw()),
                    WM_GPUI_FORCE_UPDATE_WINDOW,
                    WPARAM(window.generation()),
                    LPARAM::default(),
                )
                .log_err();
            });
        });
    }
    Ok(())
}

fn with_live_vsync_owner<R>(
    vsync_owner_live: &RwLock<bool>,
    dispatch: impl FnOnce() -> R,
) -> Option<R> {
    let owner_live = vsync_owner_live.read();
    (*owner_live).then(dispatch)
}

fn dispatch_registered_window_snapshot(
    registered_windows: &RegisteredWindows,
    snapshot: impl IntoIterator<Item = RegisteredWindow>,
    mut dispatch: impl FnMut(RegisteredWindow),
) -> SmallVec<[RegisteredWindow; 4]> {
    let mut survivors = SmallVec::new();
    for registered_window in snapshot {
        if !registered_window_is_current(registered_windows, registered_window) {
            continue;
        }
        dispatch(registered_window);
        if registered_window_is_current(registered_windows, registered_window) {
            survivors.push(registered_window);
        }
    }
    survivors
}

fn registered_window_is_current(
    registered_windows: &RegisteredWindows,
    candidate: RegisteredWindow,
) -> bool {
    registered_windows
        .read()
        .iter()
        .copied()
        .any(|registered| registered.matches(candidate))
}

fn validate_credential_blob_size(size: usize) -> Result<()> {
    if size > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
        anyhow::bail!(
            "credential blob is {size} bytes, which exceeds the Windows Credential Manager limit of {CRED_MAX_CREDENTIAL_BLOB_SIZE} bytes"
        );
    }

    Ok(())
}

const PLATFORM_WINDOW_CLASS_NAME: PCWSTR = w!("OpenGPUI::PlatformWindow");

fn register_platform_window_class() {
    let wc = WNDCLASSW {
        lpfnWndProc: Some(window_procedure),
        lpszClassName: PCWSTR(PLATFORM_WINDOW_CLASS_NAME.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };
}

unsafe extern "system" fn window_procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let params = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let creation_context = params.lpCreateParams as *mut PlatformWindowCreateContext;
        let creation_context = unsafe { &mut *creation_context };

        let Some(main_sender) = creation_context.main_sender.take() else {
            creation_context.inner = Some(Err(anyhow!("missing main sender")));
            return LRESULT(0);
        };
        creation_context.dispatcher = Some(Arc::new(WindowsDispatcher::new(
            main_sender,
            hwnd,
            creation_context.validation_number,
        )));

        return match WindowsPlatformInner::new(creation_context) {
            Ok(inner) => {
                let weak = Box::new(Rc::downgrade(&inner));
                unsafe { set_window_long(hwnd, GWLP_USERDATA, Box::into_raw(weak) as isize) };
                creation_context.inner = Some(Ok(inner));
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            Err(error) => {
                creation_context.inner = Some(Err(error));
                LRESULT(0)
            }
        };
    }

    let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) } as *mut Weak<WindowsPlatformInner>;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let inner = unsafe { &*ptr };
    let result = if let Some(inner) = inner.upgrade() {
        if cfg!(debug_assertions) {
            let inner = std::panic::AssertUnwindSafe(inner);
            match std::panic::catch_unwind(|| { inner }.handle_msg(hwnd, msg, wparam, lparam)) {
                Ok(result) => result,
                Err(_) => std::process::abort(),
            }
        } else {
            inner.handle_msg(hwnd, msg, wparam, lparam)
        }
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    };

    if msg == WM_NCDESTROY {
        unsafe { set_window_long(hwnd, GWLP_USERDATA, 0) };
        unsafe { drop(Box::from_raw(ptr)) };
    }

    result
}

#[cfg(test)]
#[path = "native_test_support.rs"]
mod native_test_support;

#[cfg(test)]
mod tests {
    use crate::{read_from_clipboard, write_to_clipboard};
    use open_gpui::{
        AppContext as _, Application, ClipboardItem, Empty, Platform as _,
        PlatformWindowCapabilities, PlatformWindowCreationCapabilities,
        PlatformWindowMutationCapabilities, WindowActivationPolicy, WindowBounds,
        WindowCoordinateSpace, WindowCreationSupport, WindowHandle, WindowId,
        WindowInitialPresentationOrder, WindowKind, WindowMutationDispatch,
        WindowMutationObservation, WindowMutationOutcome, WindowMutationRequest,
        WindowMutationSupport, WindowOptions, WindowPlacementRequest, WindowPlacementState,
        WindowPlatformFacts, px, size,
    };
    use std::{
        rc::Rc,
        sync::{Arc, mpsc},
        time::Duration,
    };
    use windows::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::{
            GetWindowRect, IsWindowVisible, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
            SetWindowPos, ShowWindow,
        },
    };

    fn observe_native_mutation(
        platform: &super::WindowsPlatform,
        app: &mut Application,
        window: WindowHandle<Empty>,
        request: WindowMutationRequest,
    ) -> WindowMutationObservation {
        let dispatch = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| window.request_window_mutation(request))
                .expect("native test window should remain open")
        });
        let ticket = match dispatch {
            WindowMutationDispatch::Queued(ticket) => ticket,
            other => panic!("expected queued native mutation, got {other:?}"),
        };

        for _ in 0..16 {
            platform.inner.run_foreground_task();
            if let Some(observation) = ticket.observation() {
                return observation;
            }
        }
        panic!(
            "native mutation did not settle: domain={:?}, generation={}",
            ticket.domain(),
            ticket.generation()
        );
    }

    fn committed_window_facts(
        app: &mut Application,
        window: WindowHandle<Empty>,
    ) -> WindowPlatformFacts {
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| window.platform_facts().clone())
                .expect("native test window should remain open")
        })
    }

    #[test]
    fn test_clipboard() {
        let item = ClipboardItem::new_string("你好，我是张小白".to_string());
        write_to_clipboard(item.clone());
        assert_eq!(read_from_clipboard(), Some(item));

        let item = ClipboardItem::new_string("12345".to_string());
        write_to_clipboard(item.clone());
        assert_eq!(read_from_clipboard(), Some(item));

        let item = ClipboardItem::new_string_with_json_metadata("abcdef".to_string(), vec![3, 4]);
        write_to_clipboard(item.clone());
        assert_eq!(read_from_clipboard(), Some(item));
    }

    #[test]
    fn credential_blob_size_error_omits_secret_context() {
        let secret_url = "https://example.test/callback?token=secret-token";
        let username = "secret-user@example.test";
        let password = b"secret-password";
        let oversized = super::CRED_MAX_CREDENTIAL_BLOB_SIZE as usize + 1;

        super::validate_credential_blob_size(password.len()).unwrap();

        let error = super::validate_credential_blob_size(oversized).unwrap_err();
        let message = error.to_string();

        assert!(message.contains(&oversized.to_string()));
        assert!(message.contains(&super::CRED_MAX_CREDENTIAL_BLOB_SIZE.to_string()));
        assert!(!message.contains(secret_url));
        assert!(!message.contains(username));
        assert!(!message.contains("secret-password"));
    }

    #[test]
    fn registered_window_dispatch_releases_registry_lock_and_observes_concurrent_removal() {
        let window_id = WindowId::from(7_u64);
        let registered_window = super::RegisteredWindow::new(HWND::default(), 7, window_id);
        let mut entries: smallvec::SmallVec<[super::RegisteredWindow; 4]> =
            smallvec::SmallVec::new();
        entries.push(registered_window);
        let registered_windows = Arc::new(parking_lot::RwLock::new(entries));
        let snapshot = registered_windows.read().clone();
        let (begin_removal_tx, begin_removal_rx) = mpsc::channel();
        let (removal_complete_tx, removal_complete_rx) = mpsc::channel();
        let remover_windows = registered_windows.clone();
        let remover = std::thread::spawn(move || {
            begin_removal_rx
                .recv()
                .expect("dispatch should request concurrent removal");
            remover_windows
                .write()
                .retain(|registered| !registered.matches(registered_window));
            removal_complete_tx
                .send(())
                .expect("dispatch should wait for concurrent removal");
        });

        let survivors =
            super::dispatch_registered_window_snapshot(&registered_windows, snapshot, |_| {
                begin_removal_tx
                    .send(())
                    .expect("remover should remain available");
                removal_complete_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("dispatch must not hold the registry read lock");
            });

        remover.join().expect("concurrent remover should finish");
        assert!(survivors.is_empty());
        assert!(registered_windows.read().is_empty());
    }

    #[test]
    fn vsync_owner_gate_rejects_dispatch_after_teardown_begins() {
        let owner_live = parking_lot::RwLock::new(true);
        let dispatches = std::sync::atomic::AtomicUsize::new(0);

        assert!(
            super::with_live_vsync_owner(&owner_live, || {
                dispatches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
            .is_some()
        );
        *owner_live.write() = false;
        assert!(
            super::with_live_vsync_owner(&owner_live, || {
                dispatches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
            .is_none()
        );
        assert_eq!(dispatches.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn unsupported_app_visibility_controls_do_not_panic() {
        let platform = super::WindowsPlatform::new(true).unwrap();

        platform.hide_other_apps();
        platform.unhide_other_apps();
    }

    #[test]
    fn window_capabilities_match_observable_windows_paths() {
        assert_eq!(
            super::windows_window_capabilities(),
            PlatformWindowCapabilities {
                creation: PlatformWindowCreationCapabilities {
                    focus_on_appearing: WindowCreationSupport::Supported,
                    transient_for: WindowCreationSupport::Supported,
                    initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
                },
                mutations: PlatformWindowMutationCapabilities {
                    position: WindowMutationSupport::CreationOnly,
                    size: WindowMutationSupport::Live,
                    windowed: WindowMutationSupport::Live,
                    maximized: WindowMutationSupport::Live,
                    fullscreen: WindowMutationSupport::Live,
                    minimized: WindowMutationSupport::Unsupported,
                    restore_bounds: WindowMutationSupport::CreationOnly,
                    pointer_input: WindowMutationSupport::Live,
                    activation_policy: WindowMutationSupport::Live,
                    alpha: WindowMutationSupport::CreationOnly,
                    topmost: WindowMutationSupport::Unsupported,
                    taskbar_visibility: WindowMutationSupport::Unsupported,
                    coordinate_space: WindowCoordinateSpace::WindowLocal,
                },
            }
        );
    }

    #[test]
    fn nonactivating_first_appearance_preserves_lifetime_activation() {
        let platform = Rc::new(super::WindowsPlatform::new(false).unwrap());
        let mut app = Application::with_platform(platform.clone());
        let window = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        kind: WindowKind::PopUp,
                        window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
            })
            .expect("non-activating popup test window should open");
        let native_window = platform
            .raw_window_handles
            .read()
            .last()
            .expect("popup test window handle should be registered")
            .as_raw();
        let native_facts = platform
            .window_from_hwnd(native_window)
            .expect("popup test window should remain registered")
            .observed_platform_facts_for_test()
            .expect("popup creation facts should remain readable from Win32");
        let committed_facts = committed_window_facts(&mut app, window);

        let creation_facts = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| window.creation_facts().clone())
                .expect("popup test window should remain open")
        });

        assert_eq!(committed_facts, native_facts);
        assert!(!creation_facts.focus_on_appearing);
        assert!(committed_facts.accepts_activation);
        assert!(committed_facts.focus_on_click);
        assert!(!committed_facts.taskbar_visible);

        let dispatches = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| {
                    [
                        window.request_activation_policy(WindowActivationPolicy::default()),
                        window.request_taskbar_visibility(false),
                        window.request_taskbar_visibility(true),
                    ]
                })
                .expect("popup test window should remain open")
        });
        assert!(matches!(dispatches[0], WindowMutationDispatch::Unchanged));
        assert!(matches!(dispatches[1], WindowMutationDispatch::Unchanged));
        assert!(matches!(dispatches[2], WindowMutationDispatch::Unsupported));

        let window = open_gpui::AnyWindowHandle::from(window);
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("popup test window should close");
        });
        platform.inner.run_foreground_task();
    }

    #[test]
    fn advertised_live_window_mutations_commit_native_observed_facts() {
        let platform = Rc::new(super::WindowsPlatform::new(false).unwrap());
        let mut app = Application::with_platform(platform.clone());
        let window = app
            .update_for_test(|cx| {
                let window_bounds = WindowBounds::centered(size(px(320.0), px(220.0)), cx);
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(window_bounds),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
            })
            .expect("hidden native test window should open");
        let native_window = platform
            .raw_window_handles
            .read()
            .last()
            .expect("native test window handle should be registered")
            .as_raw();
        assert!(unsafe { IsWindowVisible(native_window).as_bool() });

        let initial = committed_window_facts(&mut app, window);
        let native_initial = platform
            .window_from_hwnd(native_window)
            .expect("native test window should remain registered")
            .observed_platform_facts_for_test()
            .expect("creation facts should remain readable from Win32");
        assert_eq!(
            initial, native_initial,
            "the committed creation seed must equal an independent native readback"
        );
        let initial_getters = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| {
                    (
                        window.bounds(),
                        window.window_bounds(),
                        window.inner_window_bounds(),
                        window.is_maximized(),
                        window.is_minimized(),
                        window.accepts_pointer_input(),
                    )
                })
                .expect("native test window should remain open")
        });
        assert_eq!(
            initial_getters,
            (
                initial.bounds,
                initial.window_bounds,
                initial.inner_window_bounds,
                initial.is_maximized,
                initial.is_minimized,
                initial.accepts_pointer_input,
            ),
            "creation readback must seed every public getter from the committed fact cache"
        );
        let target_size = if initial.bounds.size == size(px(360.0), px(240.0)) {
            size(px(380.0), px(260.0))
        } else {
            size(px(360.0), px(240.0))
        };
        let resized = observe_native_mutation(
            &platform,
            &mut app,
            window,
            WindowMutationRequest::Placement(WindowPlacementRequest {
                size: Some(target_size),
                ..WindowPlacementRequest::new()
            }),
        );
        assert_eq!(resized.outcome, WindowMutationOutcome::Exact);
        assert_eq!(resized.facts.bounds.size, target_size);
        assert_eq!(committed_window_facts(&mut app, window), resized.facts);

        for state in [
            WindowPlacementState::Maximized,
            WindowPlacementState::Windowed,
            WindowPlacementState::Fullscreen,
            WindowPlacementState::Windowed,
        ] {
            let observed = observe_native_mutation(
                &platform,
                &mut app,
                window,
                WindowMutationRequest::Placement(WindowPlacementRequest {
                    state: Some(state),
                    ..WindowPlacementRequest::new()
                }),
            );
            assert_eq!(
                observed.outcome,
                WindowMutationOutcome::Exact,
                "native state transition should settle exactly: {state:?}"
            );
            assert_eq!(committed_window_facts(&mut app, window), observed.facts);
        }

        for accepts_pointer_input in [false, true] {
            let observed = observe_native_mutation(
                &platform,
                &mut app,
                window,
                WindowMutationRequest::PointerInput(accepts_pointer_input),
            );
            assert_eq!(observed.outcome, WindowMutationOutcome::Exact);
            assert_eq!(observed.facts.accepts_pointer_input, accepts_pointer_input);
            assert_eq!(committed_window_facts(&mut app, window), observed.facts);
        }

        for activation_policy in [
            WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: false,
            },
            WindowActivationPolicy {
                accepts_activation: true,
                focus_on_click: false,
            },
            WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: true,
            },
            WindowActivationPolicy::default(),
        ] {
            let observed = observe_native_mutation(
                &platform,
                &mut app,
                window,
                WindowMutationRequest::ActivationPolicy(activation_policy),
            );
            assert_eq!(observed.outcome, WindowMutationOutcome::Exact);
            assert_eq!(
                (
                    observed.facts.accepts_activation,
                    observed.facts.focus_on_click,
                ),
                (
                    activation_policy.accepts_activation,
                    activation_policy.focus_on_click,
                )
            );
            assert_eq!(committed_window_facts(&mut app, window), observed.facts);
        }

        let window = open_gpui::AnyWindowHandle::from(window);
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("native test window should close");
        });
        platform.inner.run_foreground_task();
    }

    #[test]
    fn native_mutation_failure_settles_rejected_and_rolls_back_facts() {
        let platform = Rc::new(super::WindowsPlatform::new(false).unwrap());
        let mut app = Application::with_platform(platform.clone());
        let window = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
            })
            .expect("native test window should open");
        let native_window = platform
            .raw_window_handles
            .read()
            .last()
            .expect("native test window handle should be registered")
            .as_raw();
        let native_window_state = platform
            .window_from_hwnd(native_window)
            .expect("native test window should remain registered");
        let initial = committed_window_facts(&mut app, window);
        native_window_state
            .state
            .fail_next_pointer_input_frame_change
            .set(true);

        let observation = observe_native_mutation(
            &platform,
            &mut app,
            window,
            WindowMutationRequest::PointerInput(!initial.accepts_pointer_input),
        );
        assert_eq!(observation.outcome, WindowMutationOutcome::Rejected);
        assert_eq!(observation.facts, initial);
        assert_eq!(committed_window_facts(&mut app, window), initial);
        assert_eq!(
            native_window_state
                .observed_platform_facts_for_test()
                .expect("rolled-back native facts should remain readable"),
            initial,
            "native style rollback and committed facts must remain coherent"
        );

        native_window_state
            .state
            .fail_next_activation_policy_frame_change
            .set(true);
        let activation_observation = observe_native_mutation(
            &platform,
            &mut app,
            window,
            WindowMutationRequest::ActivationPolicy(WindowActivationPolicy {
                accepts_activation: !initial.accepts_activation,
                focus_on_click: !initial.focus_on_click,
            }),
        );
        assert_eq!(
            activation_observation.outcome,
            WindowMutationOutcome::Rejected
        );
        assert_eq!(activation_observation.facts, initial);
        assert_eq!(committed_window_facts(&mut app, window), initial);

        let window = open_gpui::AnyWindowHandle::from(window);
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("native test window should close");
        });
        platform.inner.run_foreground_task();
    }

    #[test]
    fn native_rejection_does_not_fabricate_committed_facts() {
        let platform = Rc::new(super::WindowsPlatform::new(false).unwrap());
        let mut app = Application::with_platform(platform.clone());
        let window = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
            })
            .expect("native test window should open");
        let native_window = platform
            .raw_window_handles
            .read()
            .last()
            .expect("native test window handle should be registered")
            .as_raw();
        assert!(unsafe { IsWindowVisible(native_window).as_bool() });

        let initial = committed_window_facts(&mut app, window);
        unsafe {
            let _ = ShowWindow(native_window, SW_HIDE);
        }
        assert!(!unsafe { IsWindowVisible(native_window).as_bool() });

        let target_size = size(
            initial.bounds.size.width + px(32.0),
            initial.bounds.size.height + px(24.0),
        );
        let dispatch = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| {
                    window.request_window_mutation(WindowMutationRequest::Placement(
                        WindowPlacementRequest {
                            size: Some(target_size),
                            ..WindowPlacementRequest::new()
                        },
                    ))
                })
                .expect("native test window should remain open")
        });
        assert!(
            matches!(dispatch, WindowMutationDispatch::Rejected),
            "hidden native window should reject live placement, got {dispatch:?}"
        );
        assert_eq!(
            committed_window_facts(&mut app, window),
            initial,
            "a native dispatch rejection must not commit requested facts"
        );

        let window = open_gpui::AnyWindowHandle::from(window);
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("native test window should close");
        });
        platform.inner.run_foreground_task();
    }

    #[test]
    fn external_native_resize_callback_refreshes_committed_facts() {
        let platform = Rc::new(super::WindowsPlatform::new(false).unwrap());
        let mut app = Application::with_platform(platform.clone());
        let window = app
            .update_for_test(|cx| {
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
            })
            .expect("native test window should open");
        let native_window = platform
            .raw_window_handles
            .read()
            .last()
            .expect("native test window handle should be registered")
            .as_raw();
        let initial = committed_window_facts(&mut app, window);
        let mut outer_bounds = RECT::default();
        unsafe { GetWindowRect(native_window, &mut outer_bounds) }
            .expect("native outer bounds should be readable");

        unsafe {
            SetWindowPos(
                native_window,
                None,
                0,
                0,
                outer_bounds.right - outer_bounds.left + 32,
                outer_bounds.bottom - outer_bounds.top + 24,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .expect("external native resize should succeed");

        let mut observed = committed_window_facts(&mut app, window);
        for _ in 0..16 {
            if observed.bounds.size != initial.bounds.size {
                break;
            }
            platform.inner.run_foreground_task();
            observed = committed_window_facts(&mut app, window);
        }
        assert_ne!(
            observed.bounds.size, initial.bounds.size,
            "WM_SIZE must refresh the committed GPUI fact cache"
        );
        let getter_bounds = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| window.bounds())
                .expect("native test window should remain open")
        });
        assert_eq!(getter_bounds, observed.bounds);

        let window = open_gpui::AnyWindowHandle::from(window);
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("native test window should close");
        });
        platform.inner.run_foreground_task();
    }

    #[test]
    fn hidden_window_defers_live_placement_until_first_activation() {
        let platform = Rc::new(super::WindowsPlatform::new(false).unwrap());
        let mut app = Application::with_platform(platform.clone());
        let window = app
            .update_for_test(|cx| {
                let window_bounds = WindowBounds::centered(size(px(320.0), px(220.0)), cx);
                cx.open_window(
                    WindowOptions {
                        window_bounds: Some(window_bounds),
                        focus_on_appearing: false,
                        show: false,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
            })
            .expect("hidden native test window should open");
        let native_window = platform
            .raw_window_handles
            .read()
            .last()
            .expect("native test window handle should be registered")
            .as_raw();
        assert!(!unsafe { IsWindowVisible(native_window).as_bool() });

        let target_size = size(px(360.0), px(240.0));
        let dispatch = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| {
                    window.request_window_mutation(WindowMutationRequest::Placement(
                        WindowPlacementRequest {
                            size: Some(target_size),
                            ..WindowPlacementRequest::new()
                        },
                    ))
                })
                .expect("hidden native test window should remain open")
        });
        let ticket = match dispatch {
            WindowMutationDispatch::Queued(ticket) => ticket,
            other => panic!("expected deferred queued mutation, got {other:?}"),
        };
        platform.inner.run_foreground_task();
        assert!(ticket.observation().is_none());
        assert!(!unsafe { IsWindowVisible(native_window).as_bool() });

        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| window.activate_window())
                .expect("hidden native test window should remain open");
        });
        platform.inner.run_foreground_task();
        let observation = ticket
            .observation()
            .expect("first activation should settle deferred placement");
        assert_eq!(observation.outcome, WindowMutationOutcome::Exact);
        assert_eq!(observation.facts.bounds.size, target_size);
        assert_eq!(committed_window_facts(&mut app, window), observation.facts);
        assert!(unsafe { IsWindowVisible(native_window).as_bool() });

        let window = open_gpui::AnyWindowHandle::from(window);
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("native test window should close");
        });
        platform.inner.run_foreground_task();
    }
}
