use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
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
        Graphics::{
            Direct3D11::ID3D11Device,
            Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
            Gdi::*,
        },
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

    pub(crate) fn window_id(self) -> WindowId {
        self.window_id
    }

    pub(crate) fn matches(self, other: Self) -> bool {
        self.hwnd.as_raw() == other.hwnd.as_raw()
            && self.generation == other.generation
            && self.window_id == other.window_id
    }
}

impl PartialEq for RegisteredWindow {
    fn eq(&self, other: &Self) -> bool {
        self.matches(*other)
    }
}

impl Eq for RegisteredWindow {}

static NEXT_NATIVE_WINDOW_GENERATION: AtomicUsize = AtomicUsize::new(1);
const DISPLAY_TOPOLOGY_RETRY_TIMER_ID: usize = 1;
const DISPLAY_TOPOLOGY_RETRY_DELAY_MS: u32 = 100;

fn next_native_window_generation() -> usize {
    let generation = NEXT_NATIVE_WINDOW_GENERATION.fetch_add(1, Ordering::Relaxed);
    assert_ne!(
        generation, 0,
        "native window generation exhausted process-wide uniqueness"
    );
    generation
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsPlatformRunExit {
    TerminateProcess,
    #[cfg(any(test, feature = "test-support"))]
    ReturnToCaller,
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
    native_finalization_started: Cell<bool>,
    run_exit: WindowsPlatformRunExit,
    disable_direct_composition: bool,
    #[cfg(test)]
    lifecycle_test_probe: Rc<NativeWindowLifecycleTestProbe>,
}

pub(crate) struct WindowsPlatformInner {
    state: WindowsPlatformState,
    recovered_directx_devices: Arc<RwLock<Option<DirectXDevices>>>,
    // The below members will never change throughout the entire lifecycle of the app.
    validation_number: usize,
    main_receiver: PriorityQueueReceiver<RunnableVariant>,
    dispatcher: Arc<WindowsDispatcher>,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
    platform_handle: SafeHwnd,
    raw_window_handles: Arc<RegisteredWindows>,
    display_topology: RefCell<WindowsDisplayTopologyAuthority>,
    native_retirement: RefCell<WindowsNativeRetirementCoordinator>,
    #[cfg(test)]
    lifecycle_test_probe: Rc<NativeWindowLifecycleTestProbe>,
}

struct WindowsNativeRetirementCoordinator {
    pending_windows: SmallVec<[PendingNativeWindowFinalization; 8]>,
    finalization: Option<DeferredNativeWindowFinalization>,
    retry: NativeRetirementRetryAuthority,
}

impl Default for WindowsNativeRetirementCoordinator {
    fn default() -> Self {
        Self {
            pending_windows: SmallVec::new(),
            finalization: None,
            retry: NativeRetirementRetryAuthority::default(),
        }
    }
}

#[derive(Default)]
struct NativeRetirementRetryAuthority {
    scheduled: bool,
    generation: u64,
    attempt: usize,
}

enum PendingNativeWindowIdentity {
    Exact(Rc<WindowsWindowInner>),
    Ambiguous { _window: Rc<WindowsWindowInner> },
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingNativeWindowSource {
    Construction,
    AppOwned,
    PlatformFinalization,
}

struct PendingNativeWindowFinalization {
    registration: RegisteredWindow,
    owner_window_id: Option<WindowId>,
    identity: PendingNativeWindowIdentity,
    source: PendingNativeWindowSource,
}

/// Owns platform resources which must remain alive until every managed native owner and the
/// platform message HWND have independently reached their terminal boundaries.
struct DeferredNativeWindowFinalization {
    raw_window_handles: Arc<RegisteredWindows>,
    resources: PlatformNativeRetirementResources,
    keepalive: Option<Rc<WindowsPlatformInner>>,
    diagnostic_reported: bool,
}

struct PlatformNativeRetirementResources {
    platform_handle: HWND,
    suspend_resume_notification: Option<HPOWERNOTIFY>,
    ole_initialized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeRetirementDrainResult {
    Complete,
    Retryable,
    Blocked,
}

impl PendingNativeWindowFinalization {
    fn exact(window: Rc<WindowsWindowInner>, source: PendingNativeWindowSource) -> Self {
        Self {
            registration: window.registration,
            owner_window_id: window.native_owner_window_id(),
            identity: PendingNativeWindowIdentity::Exact(window),
            source,
        }
    }

    fn from_registered_window(registration: RegisteredWindow) -> Self {
        let (identity, owner_window_id) = match window_from_hwnd(registration.as_raw()) {
            Some(window) if window.registration.matches(registration) => {
                let owner_window_id = window.native_owner_window_id();
                (PendingNativeWindowIdentity::Exact(window), owner_window_id)
            }
            Some(window) => (
                PendingNativeWindowIdentity::Ambiguous { _window: window },
                None,
            ),
            None => (PendingNativeWindowIdentity::Unknown, None),
        };
        Self {
            registration,
            owner_window_id,
            identity,
            source: PendingNativeWindowSource::PlatformFinalization,
        }
    }

    fn refresh_registered_identity(&mut self, registered_windows: &RegisteredWindows) {
        if self.source != PendingNativeWindowSource::PlatformFinalization {
            return;
        }
        if !registered_window_is_current(registered_windows, self.registration) {
            return;
        }

        let Some(window) = window_from_hwnd(self.registration.as_raw()) else {
            self.identity = PendingNativeWindowIdentity::Unknown;
            self.owner_window_id = None;
            return;
        };
        if window.registration.matches(self.registration) {
            self.owner_window_id = window.native_owner_window_id();
            self.identity = PendingNativeWindowIdentity::Exact(window);
        } else {
            self.owner_window_id = None;
            self.identity = PendingNativeWindowIdentity::Ambiguous { _window: window };
        }
    }

    fn window_id(&self) -> WindowId {
        self.registration.window_id()
    }
}

impl WindowsNativeRetirementCoordinator {
    fn upsert(&mut self, pending: PendingNativeWindowFinalization) {
        let Some(index) = self
            .pending_windows
            .iter()
            .position(|current| current.registration.matches(pending.registration))
        else {
            self.pending_windows.push(pending);
            return;
        };

        let current = &mut self.pending_windows[index];
        let replace_identity = matches!(&current.identity, PendingNativeWindowIdentity::Unknown)
            || matches!(
                (&current.identity, &pending.identity),
                (
                    PendingNativeWindowIdentity::Ambiguous { .. },
                    PendingNativeWindowIdentity::Exact(_)
                )
            );
        if replace_identity {
            current.identity = pending.identity;
            current.owner_window_id = pending.owner_window_id;
        }
        if pending.source != PendingNativeWindowSource::PlatformFinalization {
            current.source = pending.source;
        }
    }
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
    retirement_owner: Option<Rc<WindowsPlatformInner>>,
    raw_window_handles: Option<Arc<RegisteredWindows>>,
}

impl WindowsPlatformConstructionGuard {
    fn initialize() -> Result<Self> {
        unsafe {
            OleInitialize(None).context("unable to initialize Windows OLE")?;
        }
        Ok(Self {
            hwnd: None,
            ole_initialized: true,
            retirement_owner: None,
            raw_window_handles: None,
        })
    }

    fn own_hwnd(&mut self, hwnd: HWND) {
        self.hwnd = Some(hwnd);
    }

    fn handoff_retirement(
        &mut self,
        owner: Rc<WindowsPlatformInner>,
        raw_window_handles: Arc<RegisteredWindows>,
    ) {
        self.retirement_owner = Some(owner);
        self.raw_window_handles = Some(raw_window_handles);
    }

    fn commit(mut self) {
        self.hwnd = None;
        self.ole_initialized = false;
        self.retirement_owner = None;
        self.raw_window_handles = None;
    }
}

impl Drop for WindowsPlatformConstructionGuard {
    fn drop(&mut self) {
        if let Some(owner) = self.retirement_owner.take() {
            let Some(raw_window_handles) = self.raw_window_handles.take() else {
                log::error!(
                    "Windows platform construction guard lost its registration authority; retaining managed platform resources"
                );
                return;
            };
            let Some(hwnd) = self.hwnd.take() else {
                log::error!(
                    "Windows platform construction guard lost its message HWND; retaining managed platform resources"
                );
                return;
            };
            let ole_initialized = std::mem::replace(&mut self.ole_initialized, false);
            owner.begin_platform_native_finalization(
                raw_window_handles,
                hwnd,
                None,
                ole_initialized,
            );
            return;
        }

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeWindowLifecycleTestEvent {
    PresentationQuiesced {
        window_id: WindowId,
        generation: u64,
    },
    DestroyEntered {
        window_id: WindowId,
        generation: u64,
    },
    NativeTerminal {
        window_id: WindowId,
        generation: u64,
    },
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct NativeWindowLifecycleTestProbe {
    fail_after_drag_drop_registration: Cell<bool>,
    fail_next_destroy: Cell<usize>,
    fail_next_platform_destroy: Cell<usize>,
    fail_next_suspend_resume_unregister: Cell<usize>,
    fail_next_initial_presentation: Cell<bool>,
    platform_destroy_attempts: Cell<usize>,
    suspend_resume_unregister_attempts: Cell<usize>,
    ole_uninitialize_count: Cell<usize>,
    last_created_hwnd: Cell<Option<HWND>>,
    hidden_before_map: Cell<Option<bool>>,
    initial_presentation_hook: RefCell<Option<Box<dyn FnOnce(HWND)>>>,
    provisional_compensation_before_hide_hook: RefCell<Option<Box<dyn FnOnce(HWND)>>>,
    provisional_compensation_hide_count: Cell<usize>,
    events: RefCell<Vec<NativeWindowLifecycleTestEvent>>,
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
        self.fail_destroy_attempts(1);
    }

    pub(crate) fn fail_destroy_attempts(&self, attempts: usize) {
        self.fail_next_destroy
            .set(self.fail_next_destroy.get().saturating_add(attempts));
    }

    pub(crate) fn take_fail_next_destroy(&self) -> bool {
        let attempts = self.fail_next_destroy.get();
        if attempts == 0 {
            false
        } else {
            self.fail_next_destroy.set(attempts - 1);
            true
        }
    }

    pub(crate) fn fail_next_platform_destroy(&self) {
        self.fail_platform_destroy_attempts(1);
    }

    pub(crate) fn fail_platform_destroy_attempts(&self, attempts: usize) {
        self.fail_next_platform_destroy.set(
            self.fail_next_platform_destroy
                .get()
                .saturating_add(attempts),
        );
    }

    pub(crate) fn take_fail_next_platform_destroy(&self) -> bool {
        let attempts = self.fail_next_platform_destroy.get();
        if attempts == 0 {
            false
        } else {
            self.fail_next_platform_destroy.set(attempts - 1);
            true
        }
    }

    pub(crate) fn record_platform_destroy_attempt(&self) {
        self.platform_destroy_attempts
            .set(self.platform_destroy_attempts.get().saturating_add(1));
    }

    pub(crate) fn platform_destroy_attempts(&self) -> usize {
        self.platform_destroy_attempts.get()
    }

    pub(crate) fn fail_next_suspend_resume_unregister(&self) {
        self.fail_next_suspend_resume_unregister.set(
            self.fail_next_suspend_resume_unregister
                .get()
                .saturating_add(1),
        );
    }

    pub(crate) fn take_fail_next_suspend_resume_unregister(&self) -> bool {
        let attempts = self.fail_next_suspend_resume_unregister.get();
        if attempts == 0 {
            false
        } else {
            self.fail_next_suspend_resume_unregister.set(attempts - 1);
            true
        }
    }

    pub(crate) fn record_suspend_resume_unregister_attempt(&self) {
        self.suspend_resume_unregister_attempts.set(
            self.suspend_resume_unregister_attempts
                .get()
                .saturating_add(1),
        );
    }

    pub(crate) fn suspend_resume_unregister_attempts(&self) -> usize {
        self.suspend_resume_unregister_attempts.get()
    }

    pub(crate) fn record_ole_uninitialize(&self) {
        self.ole_uninitialize_count
            .set(self.ole_uninitialize_count.get().saturating_add(1));
    }

    pub(crate) fn ole_uninitialize_count(&self) -> usize {
        self.ole_uninitialize_count.get()
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

    pub(crate) fn install_provisional_compensation_before_hide_hook(
        &self,
        hook: impl FnOnce(HWND) + 'static,
    ) {
        let mut installed = self.provisional_compensation_before_hide_hook.borrow_mut();
        assert!(
            installed.is_none(),
            "provisional-compensation test hook is already installed"
        );
        *installed = Some(Box::new(hook));
    }

    pub(crate) fn run_provisional_compensation_before_hide_hook(&self, hwnd: HWND) {
        let hook = self
            .provisional_compensation_before_hide_hook
            .borrow_mut()
            .take();
        if let Some(hook) = hook {
            hook(hwnd);
        }
    }

    pub(crate) fn record_provisional_compensation_hide(&self) {
        self.provisional_compensation_hide_count.set(
            self.provisional_compensation_hide_count
                .get()
                .saturating_add(1),
        );
    }

    pub(crate) fn provisional_compensation_hide_count(&self) -> usize {
        self.provisional_compensation_hide_count.get()
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

    pub(crate) fn record_event(&self, event: NativeWindowLifecycleTestEvent) {
        self.events.borrow_mut().push(event);
    }

    pub(crate) fn events(&self) -> Vec<NativeWindowLifecycleTestEvent> {
        self.events.borrow().clone()
    }

    pub(crate) fn clear_events(&self) {
        self.events.borrow_mut().clear();
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
        Self::new_with_run_exit(headless, WindowsPlatformRunExit::TerminateProcess)
    }

    /// Creates a Windows platform whose event loop returns to its test host after `WM_QUIT`.
    ///
    /// Production Windows applications deliberately terminate with `ExitProcess` after the event
    /// loop to avoid a known aws-lc/loader-lock teardown deadlock. Native integration tests need
    /// the process to remain alive long enough to publish an exact pre-exit HWND census, so this
    /// constructor preserves every ordinary platform behavior except that final process exit.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn new_returning_for_test(headless: bool) -> Result<Self> {
        Self::new_with_run_exit(headless, WindowsPlatformRunExit::ReturnToCaller)
    }

    fn new_with_run_exit(headless: bool, run_exit: WindowsPlatformRunExit) -> Result<Self> {
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
        #[cfg(test)]
        let lifecycle_test_probe = Rc::new(NativeWindowLifecycleTestProbe::default());

        register_platform_window_class();
        let mut context = PlatformWindowCreateContext {
            inner: None,
            validation_number,
            main_sender: Some(main_sender),
            main_receiver: Some(main_receiver),
            directx_devices,
            recovered_directx_devices: recovered_directx_devices.clone(),
            raw_window_handles: raw_window_handles.clone(),
            dispatcher: None,
            #[cfg(test)]
            lifecycle_test_probe: lifecycle_test_probe.clone(),
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
        construction_guard.handoff_retirement(inner.clone(), raw_window_handles.clone());
        context
            .dispatcher
            .take()
            .context("CreateWindowExW did not run correctly")?;

        let disable_direct_composition = std::env::var(DISABLE_DIRECT_COMPOSITION)
            .is_ok_and(|value| value == "true" || value == "1");
        let background_executor = inner.background_executor.clone();
        let foreground_executor = inner.foreground_executor.clone();

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
            native_finalization_started: Cell::new(false),
            run_exit,
            disable_direct_composition,
            drop_target_helper,
            invalidate_devices: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            lifecycle_test_probe,
        };
        construction_guard.commit();
        Ok(platform)
    }

    fn begin_native_finalization(&self) {
        if self.native_finalization_started.replace(true) {
            return;
        }
        *self.vsync_owner_live.write() = false;
        self.inner.begin_platform_native_finalization(
            self.raw_window_handles.clone(),
            self.handle,
            self.suspend_resume_notification.borrow_mut().take(),
            true,
        );
    }

    #[cfg(any(test, feature = "test-support"))]
    fn finish_native_finalization_before_return(&self) {
        const MAX_ATTEMPTS: usize = 10_000;
        const MAX_MESSAGES_PER_ATTEMPT: usize = 256;

        self.begin_native_finalization();
        for _ in 0..MAX_ATTEMPTS {
            if self.inner.native_retirement_is_settled() {
                return;
            }

            let mut processed_message = false;
            for _ in 0..MAX_MESSAGES_PER_ATTEMPT {
                let mut message = MSG::default();
                let has_message = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) };
                if !has_message.as_bool() {
                    break;
                }
                processed_message = true;
                if message.message == WM_QUIT {
                    continue;
                }
                if translate_accelerator(&message).is_none() {
                    _ = unsafe { TranslateMessage(&message) };
                    unsafe { DispatchMessageW(&message) };
                }
                if self.inner.native_retirement_is_settled() {
                    return;
                }
            }

            if !processed_message {
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        let coordinator = self.inner.native_retirement.borrow();
        panic!(
            "returning Windows test platform could not converge native finalization: pending_windows={}, finalization={}, retry_scheduled={}",
            coordinator.pending_windows.len(),
            coordinator.finalization.is_some(),
            coordinator.retry.scheduled,
        );
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

    fn generate_creation_info(
        &self,
        display_topology: WindowsDisplayTopologySnapshot,
    ) -> WindowCreationInfo {
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
            native_retirement_coordinator: Rc::downgrade(&self.inner),
            display_topology,
            recovered_directx_devices: self.recovered_directx_devices.clone(),
            disable_direct_composition: self.disable_direct_composition,
            directx_devices: self.inner.state.directx_devices.borrow().clone().unwrap(),
            invalidate_devices: self.invalidate_devices.clone(),
            #[cfg(test)]
            lifecycle_test_probe: self.lifecycle_test_probe.clone(),
        }
    }

    fn open_window_against_display_generation(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
        expected_generation: Option<u64>,
    ) -> Result<Box<dyn PlatformWindow>> {
        self.inner.refresh_display_topology_now();
        let display_topology = self.inner.exact_display_topology_snapshot().map_err(
            |unavailable| {
                anyhow!(
                    "cannot open a native window without an exact display topology: {unavailable:?}"
                )
            },
        )?;
        if let Some(expected_generation) = expected_generation {
            anyhow::ensure!(
                expected_generation == display_topology.generation(),
                "display topology changed after GPUI resolved the window creation placement"
            );
        }
        let transient_owner_hwnd = options
            .transient_for
            .map(|owner| self.native_owner_for(owner))
            .transpose()?;
        let window = WindowsWindow::new(
            handle,
            options,
            transient_owner_hwnd,
            self.generate_creation_info(display_topology),
        )?;
        self.raw_window_handles.write().push(window.0.registration);

        Ok(Box::new(window))
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
            provisional_presentation: WindowCreationSupport::Supported,
            initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
        },
        mutations: PlatformWindowMutationCapabilities {
            position: WindowMutationSupport::CreationOnly,
            physical_placement: WindowMutationSupport::Live,
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
        activation: PlatformWindowActivationSupport::Observed,
    }
}

fn registered_window_hit_candidate(
    hwnd: HWND,
    child_root: Option<HWND>,
    registered_windows: &[RegisteredWindow],
) -> Option<RegisteredWindow> {
    registered_windows
        .iter()
        .copied()
        .find(|registered| registered.as_raw() == hwnd)
        .or_else(|| {
            let root = child_root.filter(|root| !root.is_invalid() && *root != hwnd)?;
            registered_windows
                .iter()
                .copied()
                .find(|registered| registered.as_raw() == root)
        })
}

fn child_root(hwnd: HWND) -> Option<HWND> {
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    (!root.is_invalid() && root != hwnd).then_some(root)
}

fn point_is_inside_window_rect(point: Point<DevicePixels>, rect: RECT) -> bool {
    point.x.0 >= rect.left
        && point.x.0 < rect.right
        && point.y.0 >= rect.top
        && point.y.0 < rect.bottom
}

fn physical_coverage_from_native_rect(rect: RECT) -> Option<PlatformWindowPhysicalCoverage> {
    let width = rect.right.checked_sub(rect.left)?;
    let height = rect.bottom.checked_sub(rect.top)?;
    if width < 0 || height < 0 {
        return None;
    }
    PlatformWindowPhysicalCoverage::try_new(Bounds::new(
        point(DevicePixels(rect.left), DevicePixels(rect.top)),
        size(DevicePixels(width), DevicePixels(height)),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeWindowCloak {
    Uncloaked,
    Cloaked,
    Unknown,
}

fn cloak_observation(observed: Option<u32>) -> NativeWindowCloak {
    match observed {
        Some(0) => NativeWindowCloak::Uncloaked,
        Some(_) => NativeWindowCloak::Cloaked,
        None => NativeWindowCloak::Unknown,
    }
}

pub(crate) fn native_window_cloak(hwnd: HWND) -> NativeWindowCloak {
    let mut cloaked = 0_u32;
    let observed = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    }
    .ok()
    .map(|_| cloaked);
    cloak_observation(observed)
}

fn current_registered_window(
    platform: &WindowsPlatform,
    registered: RegisteredWindow,
) -> Option<Rc<WindowsWindowInner>> {
    if !registered_window_is_current(&platform.raw_window_handles, registered) {
        return None;
    }
    let window = platform.window_from_hwnd(registered.as_raw())?;
    if !window.registration.matches(registered)
        || window.handle.window_id() != registered.window_id()
    {
        return None;
    }
    Some(window)
}

fn snapshot_is_terminal_registered_hit(
    snapshot: Option<WindowProvisionalSessionSnapshot>,
    window_id: WindowId,
) -> bool {
    match snapshot {
        None => true,
        Some(snapshot) => {
            snapshot.window_id() == Some(window_id)
                && snapshot.phase() == WindowProvisionalSessionPhase::Promoted
        }
    }
}

fn registered_application_window_hit(
    platform: &WindowsPlatform,
    registered: RegisteredWindow,
    coverage: PlatformWindowPhysicalCoverage,
) -> Option<PlatformWindowHit> {
    let window = current_registered_window(platform, registered)?;
    let first_snapshot = window.provisional_session_snapshot();
    let first_accepts_pointer_input = window.state.accepts_pointer_input();
    if !snapshot_is_terminal_registered_hit(first_snapshot, registered.window_id())
        || window.provisional_requires_hit_transparency()
        || !first_accepts_pointer_input
    {
        return None;
    }
    let physical_geometry = window.physical_geometry_from_native().ok()?;

    let current_window = current_registered_window(platform, registered)?;
    if !Rc::ptr_eq(&window, &current_window)
        || current_window.provisional_session_snapshot() != first_snapshot
        || current_window.provisional_requires_hit_transparency()
        || current_window.state.accepts_pointer_input() != first_accepts_pointer_input
    {
        return None;
    }

    Some(PlatformWindowHit::RegisteredApplication {
        window: window.handle,
        coverage,
        geometry: physical_geometry,
    })
}

#[derive(Clone, Copy, Debug)]
struct NativeProvisionalWindowObservation {
    registered: RegisteredWindow,
    window: AnyWindowHandle,
    session_generation: u64,
    geometry: PlatformWindowPhysicalGeometry,
}

impl PartialEq for NativeProvisionalWindowObservation {
    fn eq(&self, other: &Self) -> bool {
        self.registered.matches(other.registered)
            && self.window == other.window
            && self.session_generation == other.session_generation
            && self.geometry == other.geometry
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NativeCoveringWindowRole {
    ProvisionalPassThrough(NativeProvisionalWindowObservation),
    NoInputPassThrough(NativeNoInputWindowObservation),
    Terminal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeNoInputWindowObservation {
    registered: RegisteredWindow,
    window: AnyWindowHandle,
    pointer_input_generation: u64,
}

#[derive(Clone, Copy, Debug)]
struct NativeCoveringWindowObservation {
    hwnd: HWND,
    child_root: Option<HWND>,
    coverage: PlatformWindowPhysicalCoverage,
    cloak: NativeWindowCloak,
    role: NativeCoveringWindowRole,
}

impl PartialEq for NativeCoveringWindowObservation {
    fn eq(&self, other: &Self) -> bool {
        self.hwnd == other.hwnd
            && self.child_root == other.child_root
            && self.coverage == other.coverage
            && self.cloak == other.cloak
            && self.role == other.role
    }
}

impl NativeCoveringWindowObservation {
    fn is_terminal(self) -> bool {
        self.role == NativeCoveringWindowRole::Terminal
    }
}

#[derive(Clone, Copy, Debug)]
enum RegisteredPointHitInspection {
    ProvisionalPassThrough(NativeProvisionalWindowObservation),
    NoInputPassThrough(NativeNoInputWindowObservation),
    Terminal,
    Failed,
}

fn inspect_registered_window_for_point(
    platform: &WindowsPlatform,
    registered: RegisteredWindow,
) -> RegisteredPointHitInspection {
    let Some(window) = current_registered_window(platform, registered) else {
        return RegisteredPointHitInspection::Failed;
    };
    let first_snapshot = window.provisional_session_snapshot();
    let first_requires_hit_transparency = window.provisional_requires_hit_transparency();
    let first_accepts_pointer_input = window.state.accepts_pointer_input();
    let first_pointer_input_generation = window.pointer_input_observation_generation();
    let provisional_geometry = match first_snapshot {
        Some(snapshot)
            if snapshot.window_id() == Some(registered.window_id())
                && snapshot.phase() == WindowProvisionalSessionPhase::Gated
                && snapshot.requires_native_hit_transparency()
                && first_requires_hit_transparency =>
        {
            let Ok(geometry) = window.physical_geometry_from_native() else {
                return RegisteredPointHitInspection::Failed;
            };
            Some((snapshot, geometry))
        }
        None if !first_requires_hit_transparency => None,
        Some(snapshot)
            if snapshot.window_id() == Some(registered.window_id())
                && snapshot.phase() == WindowProvisionalSessionPhase::Promoted
                && !snapshot.requires_native_hit_transparency()
                && !first_requires_hit_transparency =>
        {
            None
        }
        Some(_) | None => return RegisteredPointHitInspection::Failed,
    };

    let Some(current_window) = current_registered_window(platform, registered) else {
        return RegisteredPointHitInspection::Failed;
    };
    if !Rc::ptr_eq(&window, &current_window)
        || current_window.provisional_session_snapshot() != first_snapshot
        || current_window.provisional_requires_hit_transparency() != first_requires_hit_transparency
        || current_window.state.accepts_pointer_input() != first_accepts_pointer_input
        || current_window.pointer_input_observation_generation() != first_pointer_input_generation
    {
        return RegisteredPointHitInspection::Failed;
    }

    match provisional_geometry {
        Some((snapshot, geometry)) => RegisteredPointHitInspection::ProvisionalPassThrough(
            NativeProvisionalWindowObservation {
                registered,
                window: window.handle,
                session_generation: snapshot.generation(),
                geometry,
            },
        ),
        None if first_accepts_pointer_input => RegisteredPointHitInspection::Terminal,
        None => RegisteredPointHitInspection::NoInputPassThrough(NativeNoInputWindowObservation {
            registered,
            window: window.handle,
            pointer_input_generation: first_pointer_input_generation,
        }),
    }
}

fn point_covering_windows_in_z_order(
    platform: &WindowsPlatform,
    point: Point<DevicePixels>,
    registered_windows: &[RegisteredWindow],
) -> Option<Vec<NativeCoveringWindowObservation>> {
    let native_target = window_from_point_root(point);
    let mut context = NativePointBoundWindowEnumeration {
        platform,
        point,
        native_target,
        registered_windows,
        enumeration: PointBoundWindowEnumeration::new(),
    };
    // SAFETY: EnumWindows invokes the callback synchronously. The pointer remains valid for this
    // call and the stack-local context has no other aliases while the callback mutates it.
    let native_completed = unsafe {
        EnumWindows(
            Some(collect_point_bound_window),
            LPARAM(&mut context as *mut NativePointBoundWindowEnumeration<'_> as isize),
        )
        .ok()
    }
    .is_some();
    finish_point_bound_window_enumeration(
        context.enumeration,
        native_completed,
        context.native_target,
    )
}

#[derive(Clone, Copy, Debug)]
enum PointBoundWindowInspection {
    Skip,
    PassThrough(NativeCoveringWindowObservation),
    Terminal(NativeCoveringWindowObservation),
    Failed,
}

fn inspect_point_bound_window(
    platform: &WindowsPlatform,
    point: Point<DevicePixels>,
    native_target: Option<HWND>,
    hwnd: HWND,
    registered_windows: &[RegisteredWindow],
) -> PointBoundWindowInspection {
    if native_window_is_shell_desktop(hwnd) {
        return PointBoundWindowInspection::Skip;
    }
    if unsafe {
        !IsWindow(Some(hwnd)).as_bool()
            || !IsWindowVisible(hwnd).as_bool()
            || IsIconic(hwnd).as_bool()
    } {
        return PointBoundWindowInspection::Skip;
    }

    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return if unsafe { !IsWindow(Some(hwnd)).as_bool() } {
            PointBoundWindowInspection::Skip
        } else {
            PointBoundWindowInspection::Failed
        };
    }
    if !point_is_inside_window_rect(point, rect) {
        return PointBoundWindowInspection::Skip;
    }
    let Some(coverage) = physical_coverage_from_native_rect(rect) else {
        return PointBoundWindowInspection::Failed;
    };
    if unsafe {
        !IsWindow(Some(hwnd)).as_bool()
            || !IsWindowVisible(hwnd).as_bool()
            || IsIconic(hwnd).as_bool()
    } {
        return PointBoundWindowInspection::Skip;
    }
    let cloak = native_window_cloak(hwnd);
    if cloak == NativeWindowCloak::Cloaked {
        return PointBoundWindowInspection::Skip;
    }
    let child_root = child_root(hwnd);
    let observation = NativeCoveringWindowObservation {
        hwnd,
        child_root,
        coverage,
        cloak,
        role: NativeCoveringWindowRole::Terminal,
    };
    if cloak == NativeWindowCloak::Unknown {
        return classify_point_bound_window_candidate(observation, native_target, None);
    }
    let registered = registered_window_hit_candidate(hwnd, child_root, registered_windows)
        .map(|registered| inspect_registered_window_for_point(platform, registered));
    classify_point_bound_window_candidate(observation, native_target, registered)
}

fn classify_point_bound_window_candidate(
    mut observation: NativeCoveringWindowObservation,
    native_target: Option<HWND>,
    registered: Option<RegisteredPointHitInspection>,
) -> PointBoundWindowInspection {
    let is_native_target = native_target == Some(observation.hwnd);
    match registered {
        None => PointBoundWindowInspection::Terminal(observation),
        Some(RegisteredPointHitInspection::ProvisionalPassThrough(provisional)) => {
            if is_native_target {
                return PointBoundWindowInspection::Failed;
            }
            observation.role = NativeCoveringWindowRole::ProvisionalPassThrough(provisional);
            PointBoundWindowInspection::PassThrough(observation)
        }
        Some(RegisteredPointHitInspection::NoInputPassThrough(no_input)) => {
            if is_native_target {
                return PointBoundWindowInspection::Failed;
            }
            observation.role = NativeCoveringWindowRole::NoInputPassThrough(no_input);
            PointBoundWindowInspection::PassThrough(observation)
        }
        Some(RegisteredPointHitInspection::Terminal) => {
            PointBoundWindowInspection::Terminal(observation)
        }
        Some(RegisteredPointHitInspection::Failed) => PointBoundWindowInspection::Failed,
    }
}

fn classify_covering_windows(
    platform: &WindowsPlatform,
    observations: &[NativeCoveringWindowObservation],
    registered_windows: &[RegisteredWindow],
) -> Option<Vec<PlatformWindowHit>> {
    observations
        .iter()
        .map(|observation| {
            match observation.role {
                NativeCoveringWindowRole::ProvisionalPassThrough(expected) => {
                    let RegisteredPointHitInspection::ProvisionalPassThrough(current) =
                        inspect_registered_window_for_point(platform, expected.registered)
                    else {
                        return None;
                    };
                    if current != expected {
                        return None;
                    }
                    return Some(PlatformWindowHit::ProvisionalPassThrough {
                        window: expected.window,
                        session_generation: expected.session_generation,
                        coverage: observation.coverage,
                        geometry: expected.geometry,
                    });
                }
                NativeCoveringWindowRole::NoInputPassThrough(expected) => {
                    let RegisteredPointHitInspection::NoInputPassThrough(current) =
                        inspect_registered_window_for_point(platform, expected.registered)
                    else {
                        return None;
                    };
                    if current != expected {
                        return None;
                    }
                    return Some(PlatformWindowHit::NoInputPassThrough {
                        window: expected.window,
                        pointer_input_generation: expected.pointer_input_generation,
                        coverage: observation.coverage,
                    });
                }
                NativeCoveringWindowRole::Terminal => {}
            }
            if observation.cloak == NativeWindowCloak::Unknown {
                return Some(PlatformWindowHit::OpaqueBarrier {
                    coverage: observation.coverage,
                });
            }
            let registered = registered_window_hit_candidate(
                observation.hwnd,
                observation.child_root,
                registered_windows,
            );
            let Some(registered) = registered else {
                return Some(PlatformWindowHit::OpaqueBarrier {
                    coverage: observation.coverage,
                });
            };
            registered_application_window_hit(platform, registered, observation.coverage)
        })
        .collect()
}

#[cfg(feature = "test-support")]
fn apply_native_no_input_generation_drift_after_first_classification(
    platform: &WindowsPlatform,
    observations: &[NativeCoveringWindowObservation],
) {
    let Some(target) =
        crate::native_test_harness::pending_native_no_input_generation_drift_target()
    else {
        return;
    };
    let Some(expected) = observations
        .iter()
        .find_map(|observation| match observation.role {
            NativeCoveringWindowRole::NoInputPassThrough(expected)
                if expected.window.window_id() == target =>
            {
                Some(expected)
            }
            NativeCoveringWindowRole::ProvisionalPassThrough(_)
            | NativeCoveringWindowRole::NoInputPassThrough(_)
            | NativeCoveringWindowRole::Terminal => None,
        })
    else {
        return;
    };
    let Some(request) = crate::native_test_harness::take_native_no_input_generation_drift(target)
    else {
        return;
    };
    let Some(window) = current_registered_window(platform, expected.registered) else {
        request.mark_target_missing();
        return;
    };
    window.invalidate_pointer_input_observation_for_native_test();
    request.mark_applied();
}

fn stabilized_window_hit_stack(
    point: Point<DevicePixels>,
    first_observation: &[NativeCoveringWindowObservation],
    first_hits: &[PlatformWindowHit],
    second_observation: &[NativeCoveringWindowObservation],
    second_hits: &[PlatformWindowHit],
    final_observation: &[NativeCoveringWindowObservation],
    final_hits: Vec<PlatformWindowHit>,
    verified_observation: &[NativeCoveringWindowObservation],
) -> PlatformWindowHitStack {
    if !classification_is_complete_through_first_terminal(first_observation, first_hits)
        || !classification_is_complete_through_first_terminal(second_observation, second_hits)
        || !classification_is_complete_through_first_terminal(final_observation, &final_hits)
        || first_observation != second_observation
        || second_observation != final_observation
        || first_hits != second_hits
        || second_hits != final_hits.as_slice()
        || verified_observation != final_observation
    {
        return PlatformWindowHitStack::Unavailable;
    }
    if final_observation
        .last()
        .is_none_or(|observation| !observation.is_terminal())
    {
        PlatformWindowHitStack::try_available_open_desktop(point, final_hits).unwrap_or_default()
    } else {
        PlatformWindowHitStack::try_available(point, final_hits).unwrap_or_default()
    }
}

fn sample_stabilized_window_hit_stack(
    point: Point<DevicePixels>,
    observe: impl FnOnce() -> Option<Vec<NativeCoveringWindowObservation>>,
    mut classify: impl FnMut(&[NativeCoveringWindowObservation]) -> Option<Vec<PlatformWindowHit>>,
    verify: impl FnOnce() -> Option<Vec<NativeCoveringWindowObservation>>,
) -> PlatformWindowHitStack {
    let Some(first_observation) = observe() else {
        return PlatformWindowHitStack::Unavailable;
    };
    let Some(first_hits) = classify(&first_observation) else {
        return PlatformWindowHitStack::Unavailable;
    };
    let Some(verified_observation) = verify() else {
        return PlatformWindowHitStack::Unavailable;
    };
    let Some(verified_hits) = classify(&verified_observation) else {
        return PlatformWindowHitStack::Unavailable;
    };
    stabilized_window_hit_stack(
        point,
        &first_observation,
        &first_hits,
        &first_observation,
        &first_hits,
        &verified_observation,
        verified_hits,
        &verified_observation,
    )
}

fn classification_is_complete_through_first_terminal(
    observations: &[NativeCoveringWindowObservation],
    hits: &[PlatformWindowHit],
) -> bool {
    if observations.len() != hits.len() {
        return false;
    }
    let reaches_open_desktop = observations
        .last()
        .is_none_or(|observation| !observation.is_terminal());
    if reaches_open_desktop {
        if observations
            .iter()
            .any(|observation| observation.is_terminal())
        {
            return false;
        }
    } else {
        let Some((terminal, prefix)) = observations.split_last() else {
            return false;
        };
        if !terminal.is_terminal() || prefix.iter().any(|observation| observation.is_terminal()) {
            return false;
        }
    }
    observations.iter().zip(hits).all(|(observation, hit)| {
        if observation.coverage != hit.coverage() {
            return false;
        }
        match (observation.role, *hit) {
            (
                NativeCoveringWindowRole::ProvisionalPassThrough(expected),
                PlatformWindowHit::ProvisionalPassThrough {
                    window,
                    session_generation,
                    geometry,
                    ..
                },
            ) => {
                expected.window == window
                    && expected.session_generation == session_generation
                    && expected.geometry == geometry
            }
            (
                NativeCoveringWindowRole::NoInputPassThrough(expected),
                PlatformWindowHit::NoInputPassThrough {
                    window,
                    pointer_input_generation,
                    ..
                },
            ) => {
                expected.window == window
                    && expected.pointer_input_generation == pointer_input_generation
            }
            (NativeCoveringWindowRole::Terminal, hit) => hit.is_terminal(),
            (
                NativeCoveringWindowRole::ProvisionalPassThrough(_)
                | NativeCoveringWindowRole::NoInputPassThrough(_),
                _,
            ) => false,
        }
    })
}

fn window_from_point_root(point: Point<DevicePixels>) -> Option<HWND> {
    let hit = unsafe {
        WindowFromPoint(POINT {
            x: point.x.0,
            y: point.y.0,
        })
    };
    if hit.is_invalid() {
        return None;
    }
    let root = unsafe { GetAncestor(hit, GA_ROOT) };
    let root = if root.is_invalid() { hit } else { root };
    (!native_window_is_shell_desktop(root)).then_some(root)
}

pub(crate) fn native_window_is_shell_desktop(hwnd: HWND) -> bool {
    if hwnd.is_invalid() || hwnd == unsafe { GetDesktopWindow() } {
        return true;
    }
    let shell = unsafe { GetShellWindow() };
    if shell.is_invalid() {
        return false;
    }
    let mut shell_process = 0;
    let mut candidate_process = 0;
    unsafe {
        GetWindowThreadProcessId(shell, Some(&mut shell_process));
        GetWindowThreadProcessId(hwnd, Some(&mut candidate_process));
    }
    if shell_process == 0 || candidate_process != shell_process {
        return false;
    }

    let mut class_name = [0_u16; 64];
    let length = unsafe { GetClassNameW(hwnd, &mut class_name) };
    if length <= 0 {
        return false;
    }
    matches!(
        String::from_utf16_lossy(&class_name[..length as usize]).as_str(),
        "Progman" | "WorkerW"
    )
}

fn independently_verified_point_covering_windows(
    platform: &WindowsPlatform,
    point: Point<DevicePixels>,
) -> Option<Vec<NativeCoveringWindowObservation>> {
    let registered_windows = platform.raw_window_handles.read().clone();
    let window_from_point = window_from_point_root(point);
    let observations = point_covering_windows_via_z_order_walk(
        platform,
        point,
        window_from_point,
        registered_windows.as_slice(),
    )?;
    let Some(window_from_point) = window_from_point else {
        return observations
            .iter()
            .all(|observation| !observation.is_terminal())
            .then_some(observations);
    };
    let verified_suffix = point_covering_windows_from_z_order_candidate(
        platform,
        point,
        Some(window_from_point),
        registered_windows.as_slice(),
        window_from_point,
    )?;
    window_from_point_suffix_agrees(window_from_point, &observations, &verified_suffix)
        .then_some(observations)
}

fn window_from_point_suffix_agrees(
    window_from_point: HWND,
    observations: &[NativeCoveringWindowObservation],
    verified_suffix: &[NativeCoveringWindowObservation],
) -> bool {
    if let Some(index) = observations
        .iter()
        .position(|observation| observation.hwnd == window_from_point)
    {
        verified_suffix == &observations[index..]
    } else {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeHoveredWindow {
    Unavailable,
    NoWindow,
    Window(HWND),
}

fn frontmost_window_at_point(
    platform: &WindowsPlatform,
    point: Point<DevicePixels>,
) -> NativeHoveredWindow {
    let observations = independently_verified_point_covering_windows(platform, point);
    classify_native_hovered_window(observations.as_deref())
}

fn classify_native_hovered_window(
    observations: Option<&[NativeCoveringWindowObservation]>,
) -> NativeHoveredWindow {
    let Some(observations) = observations else {
        return NativeHoveredWindow::Unavailable;
    };
    observations
        .last()
        .filter(|observation| observation.is_terminal())
        .map_or(NativeHoveredWindow::NoWindow, |observation| {
            NativeHoveredWindow::Window(observation.hwnd)
        })
}

fn point_covering_windows_via_z_order_walk(
    platform: &WindowsPlatform,
    point: Point<DevicePixels>,
    native_target: Option<HWND>,
    registered_windows: &[RegisteredWindow],
) -> Option<Vec<NativeCoveringWindowObservation>> {
    let Ok(candidate) = (unsafe { GetTopWindow(None) }) else {
        return finish_point_bound_window_enumeration(
            PointBoundWindowEnumeration::new(),
            true,
            native_target,
        );
    };
    point_covering_windows_from_z_order_candidate(
        platform,
        point,
        native_target,
        registered_windows,
        candidate,
    )
}

fn point_covering_windows_from_z_order_candidate(
    platform: &WindowsPlatform,
    point: Point<DevicePixels>,
    native_target: Option<HWND>,
    registered_windows: &[RegisteredWindow],
    mut candidate: HWND,
) -> Option<Vec<NativeCoveringWindowObservation>> {
    let mut enumeration = PointBoundWindowEnumeration::new();
    loop {
        if !enumeration.visit_with(candidate, |hwnd| {
            inspect_point_bound_window(platform, point, native_target, hwnd, registered_windows)
        }) {
            return finish_point_bound_window_enumeration(enumeration, false, native_target);
        }
        let Ok(next) = (unsafe { GetWindow(candidate, GW_HWNDNEXT) }) else {
            return finish_point_bound_window_enumeration(enumeration, true, native_target);
        };
        candidate = next;
    }
}

const MAX_ENUMERATED_TOP_LEVEL_WINDOWS: usize = 4096;

fn record_bounded_window_walk_candidate(visited: &mut HashSet<usize>, hwnd: HWND) -> bool {
    visited.len() < MAX_ENUMERATED_TOP_LEVEL_WINDOWS && visited.insert(hwnd.0 as usize)
}

struct PointBoundWindowEnumeration {
    visited: HashSet<usize>,
    observations: Vec<NativeCoveringWindowObservation>,
    terminal: bool,
    failed: bool,
}

impl PointBoundWindowEnumeration {
    fn new() -> Self {
        Self {
            visited: HashSet::new(),
            observations: Vec::new(),
            terminal: false,
            failed: false,
        }
    }

    fn visit_with(
        &mut self,
        hwnd: HWND,
        inspect: impl FnOnce(HWND) -> PointBoundWindowInspection,
    ) -> bool {
        if self.terminal || self.failed {
            return false;
        }
        if !record_bounded_window_walk_candidate(&mut self.visited, hwnd) {
            self.failed = true;
            return false;
        }
        match inspect(hwnd) {
            PointBoundWindowInspection::Skip => true,
            PointBoundWindowInspection::PassThrough(observation)
                if observation.hwnd == hwnd && !observation.is_terminal() =>
            {
                self.observations.push(observation);
                true
            }
            PointBoundWindowInspection::Terminal(observation)
                if observation.hwnd == hwnd && observation.is_terminal() =>
            {
                self.observations.push(observation);
                self.terminal = true;
                false
            }
            PointBoundWindowInspection::PassThrough(_)
            | PointBoundWindowInspection::Terminal(_) => {
                self.failed = true;
                false
            }
            PointBoundWindowInspection::Failed => {
                self.failed = true;
                false
            }
        }
    }

    fn finish(self, native_completed: bool) -> Option<Vec<NativeCoveringWindowObservation>> {
        if self.failed {
            return None;
        }
        if self.terminal || native_completed {
            Some(self.observations)
        } else {
            None
        }
    }
}

fn finish_point_bound_window_enumeration(
    enumeration: PointBoundWindowEnumeration,
    native_completed: bool,
    native_target: Option<HWND>,
) -> Option<Vec<NativeCoveringWindowObservation>> {
    let observations = enumeration.finish(native_completed)?;
    match native_target {
        Some(target)
            if observations.last().is_some_and(|observation| {
                observation.hwnd == target && observation.is_terminal()
            }) =>
        {
            Some(observations)
        }
        Some(_) => None,
        None if observations
            .iter()
            .all(|observation| !observation.is_terminal()) =>
        {
            Some(observations)
        }
        None => None,
    }
}

struct NativePointBoundWindowEnumeration<'a> {
    platform: &'a WindowsPlatform,
    point: Point<DevicePixels>,
    native_target: Option<HWND>,
    registered_windows: &'a [RegisteredWindow],
    enumeration: PointBoundWindowEnumeration,
}

unsafe extern "system" fn collect_point_bound_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let context = data.0 as *mut NativePointBoundWindowEnumeration<'_>;
    // SAFETY: `point_covering_windows_in_z_order` passes a live, exclusively borrowed context and
    // EnumWindows does not retain the pointer after its synchronous callback returns.
    let context = unsafe { &mut *context };
    let platform = context.platform;
    let point = context.point;
    let native_target = context.native_target;
    let registered_windows = context.registered_windows;
    if context.enumeration.visit_with(hwnd, |hwnd| {
        inspect_point_bound_window(platform, point, native_target, hwnd, registered_windows)
    }) {
        BOOL(1)
    } else {
        BOOL(0)
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

        match self.run_exit {
            #[cfg(any(test, feature = "test-support"))]
            WindowsPlatformRunExit::ReturnToCaller => {
                // A returning test host keeps the process alive after WM_QUIT. Finish the
                // platform-owned native retirement while this thread still owns the Win32
                // message pump; otherwise a retryable DestroyWindow/resource cleanup would
                // enqueue a foreground wake that no longer has a consumer.
                self.finish_native_finalization_before_return();
            }
            WindowsPlatformRunExit::TerminateProcess => {
                // Bypass the CRT exit logic, which runs atexit handlers before calling ExitProcess.
                // aws-lc registers an atexit handler that intentionally acquires a lock without
                // releasing it. aws-lc also has thread_local objects which acquire this lock in
                // their destructor. Destructors for thread_locals run under the loader lock, so
                // there is a race condition where, if a thread exits after atexit handlers have
                // run, the TLS destructors will block indefinitely on this lock while holding the
                // loader lock. Since ExitProcess also requires the loader lock, process teardown
                // will deadlock.
                unsafe {
                    windows::Win32::System::Threading::ExitProcess(0);
                }
            }
        }
    }

    fn quit(&self) {
        self.foreground_executor()
            .spawn(async { unsafe { PostQuitMessage(0) } })
            .detach();
    }

    fn quit_after_terminal_shutdown(&self) {
        unsafe { PostQuitMessage(0) };
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
        self.display_snapshot().displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.display_snapshot().primary_display()
    }

    fn display_snapshot(&self) -> PlatformDisplaySnapshot {
        // Message-only windows do not receive WM_DISPLAYCHANGE/WM_SETTINGCHANGE broadcasts.
        // With no top-level window alive, synchronously refresh before publishing the snapshot so
        // a tray/global action can open the first replacement window against current topology.
        if self.raw_window_handles.read().is_empty() {
            self.inner.refresh_display_topology_now();
        }
        self.inner
            .retained_display_topology_snapshot()
            .platform_snapshot()
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
            .filter(|inner| !inner.state.interaction_is_quiesced())
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
        match frontmost_window_at_point(
            self,
            point(
                DevicePixels(cursor_position.x),
                DevicePixels(cursor_position.y),
            ),
        ) {
            NativeHoveredWindow::Unavailable => PlatformHoveredWindow::Unavailable,
            NativeHoveredWindow::NoWindow => PlatformHoveredWindow::NoWindow,
            NativeHoveredWindow::Window(hwnd) => PlatformHoveredWindow::from_window(
                self.window_from_hwnd(hwnd).map(|inner| inner.handle),
            ),
        }
    }

    fn window_hit_stack_at(&self, point: Point<DevicePixels>) -> PlatformWindowHitStack {
        let registered_windows = self.raw_window_handles.read().clone();
        #[cfg(feature = "test-support")]
        let mut first_classification = true;
        sample_stabilized_window_hit_stack(
            point,
            || point_covering_windows_in_z_order(self, point, registered_windows.as_slice()),
            |observations| {
                let hits =
                    classify_covering_windows(self, observations, registered_windows.as_slice());
                #[cfg(feature = "test-support")]
                if first_classification {
                    first_classification = false;
                    if hits.is_some() {
                        apply_native_no_input_generation_drift_after_first_classification(
                            self,
                            observations,
                        );
                    }
                }
                hits
            },
            || independently_verified_point_covering_windows(self, point),
        )
    }

    fn viewport_capabilities(&self) -> PlatformViewportCapabilities {
        PlatformViewportCapabilities {
            platform_viewport_windows: true,
            global_window_bounds: false,
            window_hit_stack: true,
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

    fn native_drag_hysteresis(&self) -> Option<PlatformNativeDragHysteresis> {
        let horizontal = unsafe { GetSystemMetrics(SM_CXDRAG) };
        let vertical = unsafe { GetSystemMetrics(SM_CYDRAG) };
        PlatformNativeDragHysteresis::try_new(DevicePixels(horizontal), DevicePixels(vertical))
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        self.open_window_against_display_generation(handle, options, None)
    }

    fn open_window_with_display_snapshot(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
        display_snapshot: PlatformDisplaySnapshot,
    ) -> Result<Box<dyn PlatformWindow>> {
        let expected_generation = display_snapshot.generation().ok_or_else(|| {
            anyhow!("Windows window creation requires an atomic display publication")
        })?;
        self.open_window_against_display_generation(handle, options, Some(expected_generation))
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
    fn new(context: &mut PlatformWindowCreateContext, platform_handle: HWND) -> Result<Rc<Self>> {
        let state = WindowsPlatformState::new(context.directx_devices.take());
        let dispatcher = context
            .dispatcher
            .as_ref()
            .context("missing dispatcher")?
            .clone();
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher.clone());
        Ok(Rc::new(Self {
            state,
            recovered_directx_devices: context.recovered_directx_devices.clone(),
            dispatcher,
            validation_number: context.validation_number,
            main_receiver: context
                .main_receiver
                .take()
                .context("missing main receiver")?,
            background_executor,
            foreground_executor,
            platform_handle: platform_handle.into(),
            raw_window_handles: context.raw_window_handles.clone(),
            display_topology: RefCell::new(
                WindowsDisplayTopologyAuthority::from_native().map_err(|error| {
                    anyhow!("collecting the initial Windows display topology: {error}")
                })?,
            ),
            native_retirement: RefCell::new(WindowsNativeRetirementCoordinator::default()),
            #[cfg(test)]
            lifecycle_test_probe: context.lifecycle_test_probe.clone(),
        }))
    }

    pub(crate) fn retained_display_topology_snapshot(&self) -> WindowsDisplayTopologySnapshot {
        self.display_topology.borrow().retained_snapshot()
    }

    pub(crate) fn exact_display_topology_snapshot(
        &self,
    ) -> Result<WindowsDisplayTopologySnapshot, WindowsDisplayTopologyUnavailable> {
        self.display_topology.borrow().exact_snapshot()
    }

    pub(crate) fn request_display_topology_refresh(&self) {
        let request = self.display_topology.borrow_mut().request_refresh();
        if !request.should_post_message {
            return;
        }
        if let Err(error) = unsafe {
            PostMessageW(
                Some(self.platform_handle.as_raw()),
                WM_GPUI_REFRESH_DISPLAY_TOPOLOGY,
                WPARAM(self.validation_number),
                LPARAM(0),
            )
        } {
            let failure =
                WindowsDisplayTopologyFailure::RefreshMessageRejected(error.code().0 as u32);
            self.display_topology
                .borrow_mut()
                .fail_scheduled_refresh(request.request_epoch, failure.clone());
            log::error!("cannot schedule a Windows display-topology refresh: {failure}");
            self.schedule_display_topology_retry();
        }
    }

    pub(crate) fn refresh_display_topology_now(&self) {
        let request = self.display_topology.borrow_mut().request_refresh();
        let request_epoch = self
            .display_topology
            .borrow_mut()
            .begin_scheduled_refresh()
            .unwrap_or(request.request_epoch);
        self.complete_display_topology_refresh(request_epoch);
    }

    fn complete_display_topology_refresh(&self, request_epoch: u64) {
        let candidate = WindowsDisplayTopologyAuthority::refresh_candidate_from_native();
        let refresh = self
            .display_topology
            .borrow_mut()
            .finish_refresh(request_epoch, candidate);
        self.finish_display_topology_refresh(&refresh);
    }

    fn handle_display_topology_refresh(&self) -> Option<isize> {
        let request_epoch = self.display_topology.borrow_mut().begin_scheduled_refresh();
        if let Some(request_epoch) = request_epoch {
            self.complete_display_topology_refresh(request_epoch);
        }
        Some(0)
    }

    fn finish_display_topology_refresh(&self, refresh: &WindowsDisplayTopologyRefresh) {
        match refresh {
            WindowsDisplayTopologyRefresh::Published { generation, .. }
            | WindowsDisplayTopologyRefresh::Unchanged { generation } => {
                self.cancel_display_topology_retry();
                self.notify_windows_of_display_topology(*generation);
            }
            WindowsDisplayTopologyRefresh::RetainedAfterFailure {
                generation,
                failure,
            } => {
                log::error!(
                    "retaining Windows display topology generation {} after refresh failure: {}",
                    generation,
                    failure
                );
                self.schedule_display_topology_retry();
            }
            WindowsDisplayTopologyRefresh::Superseded { .. } => {}
        }
    }

    fn cancel_display_topology_retry(&self) {
        unsafe {
            let _ = KillTimer(
                Some(self.platform_handle.as_raw()),
                DISPLAY_TOPOLOGY_RETRY_TIMER_ID,
            );
        }
    }

    fn schedule_display_topology_retry(&self) {
        let timer = unsafe {
            SetTimer(
                Some(self.platform_handle.as_raw()),
                DISPLAY_TOPOLOGY_RETRY_TIMER_ID,
                DISPLAY_TOPOLOGY_RETRY_DELAY_MS,
                None,
            )
        };
        if timer == 0 {
            log::error!(
                "cannot schedule a Windows display-topology retry: {}",
                std::io::Error::last_os_error()
            );
        }
    }

    fn handle_display_topology_retry(&self) -> Option<isize> {
        self.cancel_display_topology_retry();
        if self.display_topology.borrow().is_degraded() {
            self.refresh_display_topology_now();
        } else {
            let generation = self.retained_display_topology_snapshot().generation();
            self.notify_windows_of_display_topology(generation);
        }
        Some(0)
    }

    fn notify_windows_of_display_topology(&self, generation: u64) {
        let windows = self.raw_window_handles.read().clone();
        let mut retry_required = false;
        for window in windows {
            let Some(native_window) = window_from_hwnd(window.as_raw()) else {
                continue;
            };
            if native_window.state.display_topology_generation.get() >= generation {
                continue;
            }
            if let Err(error) = unsafe {
                PostMessageW(
                    Some(window.as_raw()),
                    WM_GPUI_DISPLAY_TOPOLOGY_PUBLISHED,
                    WPARAM(window.generation()),
                    LPARAM(0),
                )
            } {
                log::error!(
                    "cannot notify window {:?} about display topology generation {}: {}",
                    window.window_id(),
                    generation,
                    error
                );
                retry_required = true;
            }
        }
        if retry_required {
            self.schedule_display_topology_retry();
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn native_retirement_is_settled(&self) -> bool {
        let coordinator = self.native_retirement.borrow();
        coordinator.pending_windows.is_empty()
            && coordinator.finalization.is_none()
            && !coordinator.retry.scheduled
    }

    pub(crate) fn enqueue_construction_native_window(
        self: &Rc<Self>,
        window: Rc<WindowsWindowInner>,
    ) {
        self.enqueue_native_window(PendingNativeWindowFinalization::exact(
            window,
            PendingNativeWindowSource::Construction,
        ));
    }

    pub(crate) fn enqueue_app_owned_native_window(self: &Rc<Self>, window: Rc<WindowsWindowInner>) {
        self.enqueue_native_window(PendingNativeWindowFinalization::exact(
            window,
            PendingNativeWindowSource::AppOwned,
        ));
    }

    fn enqueue_native_window(self: &Rc<Self>, pending: PendingNativeWindowFinalization) {
        if matches!(
            &pending.identity,
            PendingNativeWindowIdentity::Exact(window) if window.is_native_window_terminal()
        ) {
            return;
        }
        self.native_retirement.borrow_mut().upsert(pending);
        self.schedule_native_retirement_retry();
    }

    fn begin_platform_native_finalization(
        self: &Rc<Self>,
        raw_window_handles: Arc<RegisteredWindows>,
        platform_handle: HWND,
        suspend_resume_notification: Option<HPOWERNOTIFY>,
        ole_initialized: bool,
    ) {
        let registered_windows = raw_window_handles.read().clone();
        let mut coordinator = self.native_retirement.borrow_mut();
        if coordinator.finalization.is_some() {
            log::error!(
                "attempted to begin Windows platform finalization while another finalization is pending"
            );
            return;
        }
        for registration in registered_windows {
            coordinator.upsert(PendingNativeWindowFinalization::from_registered_window(
                registration,
            ));
        }
        coordinator.finalization = Some(DeferredNativeWindowFinalization {
            raw_window_handles,
            resources: PlatformNativeRetirementResources {
                platform_handle,
                suspend_resume_notification,
                ole_initialized,
            },
            keepalive: Some(self.clone()),
            diagnostic_reported: false,
        });
        drop(coordinator);
        self.drive_native_retirement();
    }

    pub(crate) fn notify_native_window_terminal(self: &Rc<Self>, registration: RegisteredWindow) {
        let should_wake = self
            .native_retirement
            .borrow()
            .pending_windows
            .iter()
            .any(|pending| pending.registration.matches(registration));
        if should_wake {
            self.schedule_native_retirement_retry();
        }
    }

    fn schedule_native_retirement_retry(self: &Rc<Self>) {
        let (delay, generation) = {
            let mut coordinator = self.native_retirement.borrow_mut();
            if coordinator.retry.scheduled
                || (coordinator.pending_windows.is_empty() && coordinator.finalization.is_none())
            {
                return;
            }
            coordinator.retry.scheduled = true;
            coordinator.retry.generation = coordinator.retry.generation.wrapping_add(1);
            let generation = coordinator.retry.generation;
            let delay = native_retirement_retry_delay(coordinator.retry.attempt);
            (delay, generation)
        };

        let inner = self.clone();
        let background_executor = self.background_executor.clone();
        let timer = background_executor.timer(delay);
        self.foreground_executor
            .spawn(async move {
                timer.await;
                inner.native_retirement_retry_fired(generation);
            })
            .detach();
    }

    fn native_retirement_retry_fired(self: &Rc<Self>, generation: u64) {
        {
            let mut coordinator = self.native_retirement.borrow_mut();
            if !coordinator.retry.scheduled || coordinator.retry.generation != generation {
                return;
            }
            coordinator.retry.scheduled = false;
            coordinator.retry.attempt = coordinator.retry.attempt.saturating_add(1);
        }
        self.drive_native_retirement();
    }

    fn drive_native_retirement(self: &Rc<Self>) {
        match self.drain_native_retirement() {
            NativeRetirementDrainResult::Retryable => self.schedule_native_retirement_retry(),
            NativeRetirementDrainResult::Complete | NativeRetirementDrainResult::Blocked => {
                let mut coordinator = self.native_retirement.borrow_mut();
                coordinator.retry.scheduled = false;
                coordinator.retry.generation = coordinator.retry.generation.wrapping_add(1);
                coordinator.retry.attempt = 0;
            }
        }
    }

    fn drain_native_retirement(self: &Rc<Self>) -> NativeRetirementDrainResult {
        let (mut pending_windows, mut finalization) = {
            let mut coordinator = self.native_retirement.borrow_mut();
            (
                std::mem::take(&mut coordinator.pending_windows),
                coordinator.finalization.take(),
            )
        };

        if let Some(finalization) = finalization.as_ref() {
            for pending in &mut pending_windows {
                pending.refresh_registered_identity(&finalization.raw_window_handles);
            }
        }

        pending_windows.retain(|pending| match &pending.identity {
            PendingNativeWindowIdentity::Exact(window) => !window.is_native_window_terminal(),
            PendingNativeWindowIdentity::Ambiguous { .. }
            | PendingNativeWindowIdentity::Unknown => {
                if let Some(finalization) = finalization.as_ref()
                    && pending.source == PendingNativeWindowSource::PlatformFinalization
                    && !registered_window_is_current(
                        &finalization.raw_window_handles,
                        pending.registration,
                    )
                    && unsafe { !IsWindow(Some(pending.registration.as_raw())).as_bool() }
                {
                    return false;
                }
                true
            }
        });

        if pending_windows.iter().any(|pending| {
            matches!(
                &pending.identity,
                PendingNativeWindowIdentity::Ambiguous { .. }
                    | PendingNativeWindowIdentity::Unknown
            )
        }) {
            if let Some(finalization) = finalization.as_mut()
                && !finalization.diagnostic_reported
            {
                log::error!(
                    "Windows platform finalization is fail-closed: a registered HWND has no exact WindowsWindowInner identity; retaining the registration, owner authority, and platform message HWND without RevokeDragDrop or raw DestroyWindow"
                );
                finalization.diagnostic_reported = true;
            }
            self.restore_native_retirement_state(pending_windows, finalization);
            return NativeRetirementDrainResult::Blocked;
        }

        let ordered = match child_first_pending_windows(pending_windows) {
            Ok(ordered) => ordered,
            Err(cyclic) => {
                if let Some(finalization) = finalization.as_mut()
                    && !finalization.diagnostic_reported
                {
                    log::error!(
                        "Windows platform finalization is fail-closed: immutable transient-owner graph contains a cycle; retaining {} native owner(s) without destroying any member",
                        cyclic.len(),
                    );
                    finalization.diagnostic_reported = true;
                }
                self.restore_native_retirement_state(cyclic, finalization);
                return NativeRetirementDrainResult::Blocked;
            }
        };

        let mut survivors: SmallVec<[PendingNativeWindowFinalization; 8]> = SmallVec::new();
        let mut retryable = false;
        for pending in ordered {
            let window_id = pending.window_id();
            if survivors
                .iter()
                .any(|child: &PendingNativeWindowFinalization| {
                    child.owner_window_id == Some(window_id)
                })
            {
                retryable = true;
                survivors.push(pending);
                continue;
            }

            let PendingNativeWindowIdentity::Exact(window) = &pending.identity else {
                unreachable!("identity ambiguity must be handled before owner ordering");
            };
            if !window.destroy_native_window() && !window.is_native_window_terminal() {
                retryable = true;
                survivors.push(pending);
            } else if !window.is_native_window_terminal() {
                retryable = true;
                survivors.push(pending);
            }
        }

        {
            let mut coordinator = self.native_retirement.borrow_mut();
            let reentrant = std::mem::take(&mut coordinator.pending_windows);
            for pending in survivors {
                coordinator.upsert(pending);
            }
            for pending in reentrant {
                coordinator.upsert(pending);
            }
        }

        let has_pending = !self.native_retirement.borrow().pending_windows.is_empty();
        if has_pending {
            if let Some(finalization) = finalization.as_mut()
                && !finalization.diagnostic_reported
            {
                log::error!(
                    "Windows platform finalization retained managed native owners after a retryable rejection; exact shutdown tickets and child-first owner authority remain queued"
                );
                finalization.diagnostic_reported = true;
            }
            self.restore_native_retirement_finalization(finalization);
            return if retryable {
                NativeRetirementDrainResult::Retryable
            } else {
                NativeRetirementDrainResult::Blocked
            };
        }

        let Some(mut finalization_state) = finalization else {
            return NativeRetirementDrainResult::Complete;
        };
        if !self.try_finalize_platform_resources(&mut finalization_state.resources) {
            self.restore_native_retirement_finalization(Some(finalization_state));
            return NativeRetirementDrainResult::Retryable;
        }

        let keepalive = finalization_state.keepalive.take();
        self.restore_native_retirement_finalization(None);
        drop(keepalive);
        NativeRetirementDrainResult::Complete
    }

    fn restore_native_retirement_state(
        &self,
        pending_windows: SmallVec<[PendingNativeWindowFinalization; 8]>,
        finalization: Option<DeferredNativeWindowFinalization>,
    ) {
        let mut coordinator = self.native_retirement.borrow_mut();
        for pending in pending_windows {
            coordinator.upsert(pending);
        }
        coordinator.finalization = finalization;
    }

    fn restore_native_retirement_finalization(
        &self,
        finalization: Option<DeferredNativeWindowFinalization>,
    ) {
        self.native_retirement.borrow_mut().finalization = finalization;
    }

    fn try_finalize_platform_resources(
        &self,
        resources: &mut PlatformNativeRetirementResources,
    ) -> bool {
        if let Some(notification) = resources.suspend_resume_notification {
            #[cfg(test)]
            {
                self.lifecycle_test_probe
                    .record_suspend_resume_unregister_attempt();
                if self
                    .lifecycle_test_probe
                    .take_fail_next_suspend_resume_unregister()
                {
                    log::error!(
                        "injected suspend/resume notification unregistration failure; retaining platform retirement authority"
                    );
                    return false;
                }
            }
            // SAFETY: notification was returned by RegisterSuspendResumeNotification.
            if let Err(error) = unsafe { UnregisterSuspendResumeNotification(notification) } {
                log::error!("failed to unregister suspend/resume notification: {error}");
                return false;
            }
            resources.suspend_resume_notification = None;
        }

        // Keep the message HWND alive until every fallible platform-resource cleanup has
        // completed. It is the foreground retry transport for returning test applications.
        if unsafe { IsWindow(Some(resources.platform_handle)).as_bool() } {
            #[cfg(test)]
            {
                self.lifecycle_test_probe.record_platform_destroy_attempt();
                if self.lifecycle_test_probe.take_fail_next_platform_destroy() {
                    log::error!(
                        "injected platform message HWND destruction failure; retaining platform retirement authority"
                    );
                    return false;
                }
            }
            if let Err(error) = unsafe { DestroyWindow(resources.platform_handle) } {
                log::error!("failed to destroy platform message HWND: {error}");
            }
            if unsafe { IsWindow(Some(resources.platform_handle)).as_bool() } {
                return false;
            }
        }

        if resources.ole_initialized {
            unsafe { OleUninitialize() };
            #[cfg(test)]
            self.lifecycle_test_probe.record_ole_uninitialize();
            resources.ole_initialized = false;
        }
        true
    }

    /// Calls `project` to project to the corresponding callback field, removes it from callbacks, calls `f` with the callback and then puts the callback back.
    fn with_callback<T>(
        &self,
        project: impl Fn(&PlatformCallbacks) -> &Cell<Option<T>>,
        f: impl FnOnce(&mut T),
    ) {
        let _ = with_windows_callback(project(&self.state.callbacks), f);
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
            | WM_GPUI_GPU_DEVICE_LOST
            | WM_GPUI_REFRESH_DISPLAY_TOPOLOGY => self.handle_gpui_events(msg, wparam, lparam),
            WM_TIMER if wparam.0 == DISPLAY_TOPOLOGY_RETRY_TIMER_ID => {
                self.handle_display_topology_retry()
            }
            WM_POWERBROADCAST => self.handle_power_broadcast(wparam),
            _ => None,
        };
        if let Some(result) = handled {
            LRESULT(result)
        } else {
            unsafe { DefWindowProcW(handle, msg, wparam, lparam) }
        }
    }

    fn handle_gpui_events(
        self: &Rc<Self>,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<isize> {
        if wparam.0 != self.validation_number {
            log::error!("Wrong validation number while processing message: {message}");
            return None;
        }
        match message {
            WM_GPUI_TASK_DISPATCHED_ON_MAIN_THREAD => self.run_foreground_task(),
            WM_GPUI_DOCK_MENU_ACTION => self.handle_dock_action_event(lparam.0 as _),
            WM_GPUI_KEYBOARD_LAYOUT_CHANGED => self.handle_keyboard_layout_change(),
            WM_GPUI_GPU_DEVICE_LOST => self.handle_device_lost(),
            WM_GPUI_REFRESH_DISPLAY_TOPOLOGY => self.handle_display_topology_refresh(),
            _ => unreachable!(),
        }
    }

    #[inline]
    fn run_foreground_task(self: &Rc<Self>) -> Option<isize> {
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

        self.drive_native_retirement();
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
        self.begin_native_finalization();
    }
}

fn native_retirement_retry_delay(attempt: usize) -> Duration {
    match attempt {
        0 => Duration::ZERO,
        1 => Duration::from_millis(2),
        2 => Duration::from_millis(8),
        3 => Duration::from_millis(32),
        4 => Duration::from_millis(128),
        _ => Duration::from_millis(500),
    }
}

fn child_first_pending_windows(
    pending_windows: SmallVec<[PendingNativeWindowFinalization; 8]>,
) -> Result<
    SmallVec<[PendingNativeWindowFinalization; 8]>,
    SmallVec<[PendingNativeWindowFinalization; 8]>,
> {
    let mut remaining_indices: SmallVec<[usize; 8]> = (0..pending_windows.len()).collect();
    let mut order: SmallVec<[usize; 8]> = SmallVec::with_capacity(pending_windows.len());
    while !remaining_indices.is_empty() {
        let Some(position) = remaining_indices.iter().position(|candidate_index| {
            let candidate = &pending_windows[*candidate_index];
            !remaining_indices.iter().any(|other_index| {
                pending_windows[*other_index].owner_window_id == Some(candidate.window_id())
            })
        }) else {
            return Err(pending_windows);
        };
        order.push(remaining_indices.remove(position));
    }

    let mut pending_by_index: SmallVec<[Option<PendingNativeWindowFinalization>; 8]> =
        pending_windows.into_iter().map(Some).collect();
    let mut ordered: SmallVec<[PendingNativeWindowFinalization; 8]> =
        SmallVec::with_capacity(pending_by_index.len());
    for index in order {
        ordered.push(
            pending_by_index[index]
                .take()
                .expect("child-first owner graph indices must be unique"),
        );
    }
    Ok(ordered)
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
    pub(crate) native_retirement_coordinator: std::rc::Weak<WindowsPlatformInner>,
    pub(crate) display_topology: WindowsDisplayTopologySnapshot,
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
    raw_window_handles: Arc<RegisteredWindows>,
    dispatcher: Option<Arc<WindowsDispatcher>>,
    #[cfg(test)]
    lifecycle_test_probe: Rc<NativeWindowLifecycleTestProbe>,
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
    let dispatch = catch_unwind(AssertUnwindSafe(|| unsafe {
        platform_window_procedure_inner(hwnd, msg, wparam, lparam)
    }));
    match dispatch {
        Ok(result) => result,
        Err(payload) => {
            log::error!(
                "caught a panic at the Win32 platform-procedure boundary for message {msg}"
            );
            if matches!(msg, WM_NCCREATE | WM_NCDESTROY) {
                let ptr = unsafe { get_window_long(hwnd, GWLP_USERDATA) }
                    as *mut Weak<WindowsPlatformInner>;
                if !ptr.is_null()
                    && let Err(finalizer_payload) = catch_unwind(AssertUnwindSafe(|| unsafe {
                        release_platform_window_owner(hwnd, ptr);
                    }))
                {
                    log::error!(
                        "Win32 platform-window owner finalization panicked for message {msg}"
                    );
                    std::mem::forget(finalizer_payload);
                }
            }
            std::mem::forget(payload);
            LRESULT(0)
        }
    }
}

unsafe fn platform_window_procedure_inner(
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

        return match WindowsPlatformInner::new(creation_context, hwnd) {
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
        inner.handle_msg(hwnd, msg, wparam, lparam)
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    };

    if msg == WM_NCDESTROY {
        unsafe { release_platform_window_owner(hwnd, ptr) };
    }

    result
}

unsafe fn release_platform_window_owner(hwnd: HWND, ptr: *mut Weak<WindowsPlatformInner>) {
    unsafe { set_window_long(hwnd, GWLP_USERDATA, 0) };
    unsafe { drop(Box::from_raw(ptr)) };
}

#[cfg(test)]
#[path = "native_test_support.rs"]
mod native_test_support;

#[cfg(test)]
mod tests {
    use crate::{read_from_clipboard, write_to_clipboard};
    use open_gpui::{
        AnyWindowHandle, AppContext as _, Application, Bounds, ClipboardItem, DevicePixels, Empty,
        Platform as _, PlatformWindowActivationSupport, PlatformWindowCapabilities,
        PlatformWindowCreationCapabilities, PlatformWindowHit, PlatformWindowHitStack,
        PlatformWindowMutationCapabilities, PlatformWindowPhysicalCoverage,
        PlatformWindowPhysicalGeometry, WindowActivationPolicy, WindowBounds,
        WindowCoordinateSpace, WindowCreationSupport, WindowHandle, WindowId,
        WindowInitialPresentationOrder, WindowKind, WindowMutationDispatch,
        WindowMutationObservation, WindowMutationOutcome, WindowMutationRequest,
        WindowMutationSupport, WindowOptions, WindowPhysicalPlacementRequest,
        WindowPlacementRequest, WindowPlacementState, WindowPlatformFacts, point, px, size,
    };
    use std::{
        rc::Rc,
        sync::{Arc, mpsc},
        time::Duration,
    };
    use windows::Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        UI::WindowsAndMessaging::{
            GetClientRect, GetWindowRect, IsWindowVisible, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOZORDER, SetWindowPos, ShowWindow,
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
    fn direct_registered_window_hit_precedes_child_root_normalization() {
        let child = HWND(0x11usize as *mut core::ffi::c_void);
        let root = HWND(0x22usize as *mut core::ffi::c_void);
        let child_registered = super::RegisteredWindow::new(child, 11, WindowId::from(11_u64));
        let root_registered = super::RegisteredWindow::new(root, 22, WindowId::from(22_u64));

        let hit = super::registered_window_hit_candidate(
            child,
            Some(root),
            &[root_registered, child_registered],
        )
        .expect("the exact registered HWND should win");

        assert!(hit.matches(child_registered));
    }

    #[test]
    fn unregistered_child_hit_may_normalize_only_to_its_ga_root() {
        let child = HWND(0x33usize as *mut core::ffi::c_void);
        let root = HWND(0x44usize as *mut core::ffi::c_void);
        let root_registered = super::RegisteredWindow::new(root, 44, WindowId::from(44_u64));

        let hit = super::registered_window_hit_candidate(child, Some(root), &[root_registered])
            .expect("an unregistered child may normalize to its GA_ROOT");

        assert!(hit.matches(root_registered));
    }

    #[test]
    fn unregistered_top_level_hit_remains_an_opaque_barrier() {
        let top_level = HWND(0x55usize as *mut core::ffi::c_void);
        let native_owner = HWND(0x66usize as *mut core::ffi::c_void);
        let owner_registered =
            super::RegisteredWindow::new(native_owner, 66, WindowId::from(66_u64));

        let hit = super::registered_window_hit_candidate(top_level, None, &[owner_registered]);

        assert!(hit.is_none(), "native ownership must not bypass a barrier");
    }

    #[test]
    fn registered_window_snapshot_rejects_same_hwnd_replacement_generation() {
        let hwnd = HWND(0x77usize as *mut core::ffi::c_void);
        let window_id = WindowId::from(77_u64);
        let snapshot = super::RegisteredWindow::new(hwnd, 7, window_id);
        let replacement = super::RegisteredWindow::new(hwnd, 8, window_id);
        let mut entries: smallvec::SmallVec<[super::RegisteredWindow; 4]> =
            smallvec::SmallVec::new();
        entries.push(replacement);
        let registered_windows = parking_lot::RwLock::new(entries);

        assert!(!super::registered_window_is_current(
            &registered_windows,
            snapshot
        ));
    }

    #[test]
    fn registered_window_snapshot_rejects_same_hwnd_generation_with_another_window_id() {
        let hwnd = HWND(0x78usize as *mut core::ffi::c_void);
        let snapshot = super::RegisteredWindow::new(hwnd, 7, WindowId::from(78_u64));
        let different_window_id = super::RegisteredWindow::new(hwnd, 7, WindowId::from(79_u64));
        let mut entries: smallvec::SmallVec<[super::RegisteredWindow; 4]> =
            smallvec::SmallVec::new();
        entries.push(different_window_id);
        let registered_windows = parking_lot::RwLock::new(entries);

        assert!(!super::registered_window_is_current(
            &registered_windows,
            snapshot
        ));
    }

    #[test]
    fn hit_candidate_does_not_claim_a_registration_created_after_the_snapshot() {
        let hwnd = HWND(0x88usize as *mut core::ffi::c_void);
        let late_registration = super::RegisteredWindow::new(hwnd, 8, WindowId::from(88_u64));
        let mut entries: smallvec::SmallVec<[super::RegisteredWindow; 4]> =
            smallvec::SmallVec::new();
        entries.push(late_registration);
        let registered_windows = parking_lot::RwLock::new(entries);

        assert!(super::registered_window_is_current(
            &registered_windows,
            late_registration
        ));
        assert!(super::registered_window_hit_candidate(hwnd, None, &[]).is_none());
    }

    #[test]
    fn native_cloak_observation_distinguishes_visible_cloaked_and_unknown() {
        assert_eq!(
            super::cloak_observation(Some(0)),
            super::NativeWindowCloak::Uncloaked
        );
        assert_eq!(
            super::cloak_observation(Some(1)),
            super::NativeWindowCloak::Cloaked
        );
        assert_eq!(
            super::cloak_observation(None),
            super::NativeWindowCloak::Unknown
        );
    }

    #[test]
    fn point_bound_window_enumeration_fails_closed_on_duplicates_and_overflow() {
        let hwnd = HWND(0x89usize as *mut core::ffi::c_void);
        let mut duplicate = super::PointBoundWindowEnumeration::new();
        assert!(duplicate.visit_with(hwnd, |_| super::PointBoundWindowInspection::Skip));
        assert!(!duplicate.visit_with(hwnd, |_| super::PointBoundWindowInspection::Skip));
        assert!(duplicate.finish(true).is_none());

        let mut overflowing = super::PointBoundWindowEnumeration::new();
        for raw in 1..=super::MAX_ENUMERATED_TOP_LEVEL_WINDOWS {
            assert!(
                overflowing.visit_with(HWND(raw as *mut core::ffi::c_void), |_| {
                    super::PointBoundWindowInspection::Skip
                },)
            );
        }
        assert!(!overflowing.visit_with(
            HWND((super::MAX_ENUMERATED_TOP_LEVEL_WINDOWS + 1) as *mut core::ffi::c_void),
            |_| super::PointBoundWindowInspection::Skip,
        ));
        assert!(overflowing.finish(true).is_none());

        let mut native_failure = super::PointBoundWindowEnumeration::new();
        assert!(native_failure.visit_with(hwnd, |_| super::PointBoundWindowInspection::Skip));
        assert!(native_failure.finish(false).is_none());

        let mut native_completion = super::PointBoundWindowEnumeration::new();
        assert!(native_completion.visit_with(hwnd, |_| super::PointBoundWindowInspection::Skip));
        assert_eq!(
            native_completion
                .finish(true)
                .expect("a completed enumeration should return its empty terminal prefix"),
            Vec::new()
        );
    }

    #[test]
    fn frontmost_point_verifier_bounds_and_cycle_checks_its_z_order_walk() {
        let first = HWND(0x8dusize as *mut core::ffi::c_void);
        let mut duplicate = std::collections::HashSet::new();
        assert!(super::record_bounded_window_walk_candidate(
            &mut duplicate,
            first
        ));
        assert!(!super::record_bounded_window_walk_candidate(
            &mut duplicate,
            first
        ));

        let mut overflowing = std::collections::HashSet::new();
        for raw in 1..=super::MAX_ENUMERATED_TOP_LEVEL_WINDOWS {
            assert!(super::record_bounded_window_walk_candidate(
                &mut overflowing,
                HWND(raw as *mut core::ffi::c_void),
            ));
        }
        assert!(!super::record_bounded_window_walk_candidate(
            &mut overflowing,
            HWND((super::MAX_ENUMERATED_TOP_LEVEL_WINDOWS + 1) as *mut core::ffi::c_void),
        ));
    }

    #[test]
    fn physical_hit_rect_uses_negative_origins_and_half_open_edges() {
        let rect = RECT {
            left: -100,
            top: -50,
            right: 10,
            bottom: 20,
        };

        assert!(super::point_is_inside_window_rect(
            point(DevicePixels(-100), DevicePixels(-50)),
            rect
        ));
        assert!(super::point_is_inside_window_rect(
            point(DevicePixels(9), DevicePixels(19)),
            rect
        ));
        assert!(!super::point_is_inside_window_rect(
            point(DevicePixels(10), DevicePixels(19)),
            rect
        ));
        assert!(!super::point_is_inside_window_rect(
            point(DevicePixels(9), DevicePixels(20)),
            rect
        ));
        let coverage = super::physical_coverage_from_native_rect(rect)
            .expect("the native rectangle should be representable");
        assert_eq!(
            coverage.bounds(),
            open_gpui::Bounds::new(
                point(DevicePixels(-100), DevicePixels(-50)),
                size(DevicePixels(110), DevicePixels(70)),
            )
        );
    }

    #[test]
    fn physical_hit_geometry_rejects_inversion_and_integer_overflow() {
        assert!(
            super::physical_coverage_from_native_rect(RECT {
                left: 10,
                top: 0,
                right: 9,
                bottom: 1,
            })
            .is_none()
        );
        assert!(
            super::physical_coverage_from_native_rect(RECT {
                left: i32::MIN,
                top: 0,
                right: i32::MAX,
                bottom: 1,
            })
            .is_none()
        );
        let overflowing_bounds = open_gpui::Bounds::new(
            point(DevicePixels(i32::MAX), DevicePixels(0)),
            size(DevicePixels(1), DevicePixels(1)),
        );
        assert!(PlatformWindowPhysicalCoverage::try_new(overflowing_bounds).is_none());
        assert!(PlatformWindowPhysicalGeometry::try_new(overflowing_bounds, 1.0).is_none());
    }

    fn checked_coverage(
        origin: open_gpui::Point<DevicePixels>,
        dimensions: open_gpui::Size<DevicePixels>,
    ) -> PlatformWindowPhysicalCoverage {
        PlatformWindowPhysicalCoverage::try_new(open_gpui::Bounds::new(origin, dimensions))
            .expect("test coverage should be representable")
    }

    fn checked_geometry(
        origin: open_gpui::Point<DevicePixels>,
        dimensions: open_gpui::Size<DevicePixels>,
        scale_factor: f32,
    ) -> PlatformWindowPhysicalGeometry {
        PlatformWindowPhysicalGeometry::try_new(
            open_gpui::Bounds::new(origin, dimensions),
            scale_factor,
        )
        .expect("test geometry should be representable")
    }

    fn covering_observation(
        hwnd: HWND,
        coverage: PlatformWindowPhysicalCoverage,
    ) -> super::NativeCoveringWindowObservation {
        super::NativeCoveringWindowObservation {
            hwnd,
            child_root: None,
            coverage,
            cloak: super::NativeWindowCloak::Uncloaked,
            role: super::NativeCoveringWindowRole::Terminal,
        }
    }

    fn provisional_observation(
        hwnd: HWND,
        native_generation: usize,
        window: AnyWindowHandle,
        session_generation: u64,
        coverage: PlatformWindowPhysicalCoverage,
        geometry: PlatformWindowPhysicalGeometry,
    ) -> super::NativeCoveringWindowObservation {
        super::NativeCoveringWindowObservation {
            hwnd,
            child_root: None,
            coverage,
            cloak: super::NativeWindowCloak::Uncloaked,
            role: super::NativeCoveringWindowRole::ProvisionalPassThrough(
                super::NativeProvisionalWindowObservation {
                    registered: super::RegisteredWindow::new(
                        hwnd,
                        native_generation,
                        window.window_id(),
                    ),
                    window,
                    session_generation,
                    geometry,
                },
            ),
        }
    }

    fn provisional_hit(observation: super::NativeCoveringWindowObservation) -> PlatformWindowHit {
        let super::NativeCoveringWindowRole::ProvisionalPassThrough(provisional) = observation.role
        else {
            panic!("test observation should be provisional")
        };
        PlatformWindowHit::ProvisionalPassThrough {
            window: provisional.window,
            session_generation: provisional.session_generation,
            coverage: observation.coverage,
            geometry: provisional.geometry,
        }
    }

    fn no_input_observation(
        hwnd: HWND,
        native_generation: usize,
        window: AnyWindowHandle,
        pointer_input_generation: u64,
        coverage: PlatformWindowPhysicalCoverage,
    ) -> super::NativeCoveringWindowObservation {
        super::NativeCoveringWindowObservation {
            hwnd,
            child_root: None,
            coverage,
            cloak: super::NativeWindowCloak::Uncloaked,
            role: super::NativeCoveringWindowRole::NoInputPassThrough(
                super::NativeNoInputWindowObservation {
                    registered: super::RegisteredWindow::new(
                        hwnd,
                        native_generation,
                        window.window_id(),
                    ),
                    window,
                    pointer_input_generation,
                },
            ),
        }
    }

    fn no_input_hit(observation: super::NativeCoveringWindowObservation) -> PlatformWindowHit {
        let super::NativeCoveringWindowRole::NoInputPassThrough(no_input) = observation.role else {
            panic!("test observation should be no-input pass-through")
        };
        PlatformWindowHit::NoInputPassThrough {
            window: no_input.window,
            pointer_input_generation: no_input.pointer_input_generation,
            coverage: observation.coverage,
        }
    }

    fn stabilize_identical_observations(
        sampled_point: open_gpui::Point<DevicePixels>,
        observations: &[super::NativeCoveringWindowObservation],
        hits: Vec<PlatformWindowHit>,
        verified_observations: &[super::NativeCoveringWindowObservation],
    ) -> PlatformWindowHitStack {
        super::stabilized_window_hit_stack(
            sampled_point,
            observations,
            &hits,
            observations,
            &hits,
            observations,
            hits.clone(),
            verified_observations,
        )
    }

    #[test]
    fn point_bound_sampling_stops_at_the_first_terminal_before_independent_verification() {
        let skipped_hwnd = HWND(0x8ausize as *mut core::ffi::c_void);
        let terminal_hwnd = HWND(0x8busize as *mut core::ffi::c_void);
        let unreachable_hwnd = HWND(0x8cusize as *mut core::ffi::c_void);
        let sampled_point = point(DevicePixels(12), DevicePixels(18));
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(40), DevicePixels(40)),
        );
        let terminal_observation = covering_observation(terminal_hwnd, coverage);
        let observation_rounds = std::cell::Cell::new(0);
        let inspected_windows = std::cell::Cell::new(0);
        let verification_rounds = std::cell::Cell::new(0);

        let stack = super::sample_stabilized_window_hit_stack(
            sampled_point,
            || {
                observation_rounds.set(observation_rounds.get() + 1);
                let mut enumeration = super::PointBoundWindowEnumeration::new();
                for hwnd in [skipped_hwnd, terminal_hwnd, unreachable_hwnd] {
                    let should_continue = enumeration.visit_with(hwnd, |candidate| {
                        inspected_windows.set(inspected_windows.get() + 1);
                        if candidate == skipped_hwnd {
                            super::PointBoundWindowInspection::Skip
                        } else if candidate == terminal_hwnd {
                            super::PointBoundWindowInspection::Terminal(terminal_observation)
                        } else {
                            panic!("enumeration continued past the first terminal window")
                        }
                    });
                    if !should_continue {
                        break;
                    }
                }
                // EnumWindows reports FALSE when our callback intentionally stops at the terminal.
                enumeration.finish(false)
            },
            |observations| {
                Some(
                    observations
                        .iter()
                        .map(|observation| PlatformWindowHit::OpaqueBarrier {
                            coverage: observation.coverage,
                        })
                        .collect(),
                )
            },
            || {
                verification_rounds.set(verification_rounds.get() + 1);
                Some(vec![terminal_observation])
            },
        );

        assert_eq!(observation_rounds.get(), 1);
        assert_eq!(inspected_windows.get(), 2);
        assert_eq!(verification_rounds.get(), 1);
        assert_eq!(
            stack,
            PlatformWindowHitStack::try_available(
                sampled_point,
                vec![PlatformWindowHit::OpaqueBarrier { coverage }],
            )
            .expect("independently matching point-bound samples should produce an available stack")
        );
    }

    #[test]
    fn hit_stack_stabilization_accepts_multiple_exact_pass_through_entries() {
        let sampled_point = point(DevicePixels(20), DevicePixels(20));
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let second_geometry = checked_geometry(
            point(DevicePixels(4), DevicePixels(4)),
            size(DevicePixels(80), DevicePixels(80)),
            1.25,
        );
        let terminal_geometry = checked_geometry(
            point(DevicePixels(6), DevicePixels(6)),
            size(DevicePixels(70), DevicePixels(70)),
            1.5,
        );
        let first_window =
            AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(201_u64)));
        let second_window =
            AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(202_u64)));
        let terminal_window =
            AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(203_u64)));
        let first = no_input_observation(
            HWND(0xc1usize as *mut core::ffi::c_void),
            21,
            first_window,
            31,
            coverage,
        );
        let second = provisional_observation(
            HWND(0xc2usize as *mut core::ffi::c_void),
            22,
            second_window,
            32,
            coverage,
            second_geometry,
        );
        let terminal = covering_observation(HWND(0xc3usize as *mut core::ffi::c_void), coverage);
        let mut enumeration = super::PointBoundWindowEnumeration::new();
        assert!(enumeration.visit_with(first.hwnd, |_| {
            super::PointBoundWindowInspection::PassThrough(first)
        }));
        assert!(enumeration.visit_with(second.hwnd, |_| {
            super::PointBoundWindowInspection::PassThrough(second)
        }));
        assert!(!enumeration.visit_with(terminal.hwnd, |_| {
            super::PointBoundWindowInspection::Terminal(terminal)
        }));
        let observations = enumeration
            .finish(false)
            .expect("the exact prefix should finish at its first terminal");
        assert_eq!(observations, vec![first, second, terminal]);

        let hits = vec![
            no_input_hit(first),
            provisional_hit(second),
            PlatformWindowHit::RegisteredApplication {
                window: terminal_window,
                coverage,
                geometry: terminal_geometry,
            },
        ];
        assert_eq!(
            stabilize_identical_observations(
                sampled_point,
                &observations,
                hits.clone(),
                &observations,
            ),
            PlatformWindowHitStack::try_available(sampled_point, hits)
                .expect("the complete exact prefix should be available")
        );
    }

    #[test]
    fn completed_provisional_prefix_can_terminate_at_verified_open_desktop() {
        let sampled_point = point(DevicePixels(20), DevicePixels(20));
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let geometry = checked_geometry(
            point(DevicePixels(2), DevicePixels(2)),
            size(DevicePixels(90), DevicePixels(90)),
            1.0,
        );
        let window = AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(204_u64)));
        let provisional = provisional_observation(
            HWND(0xc4usize as *mut core::ffi::c_void),
            23,
            window,
            33,
            coverage,
            geometry,
        );
        let mut enumeration = super::PointBoundWindowEnumeration::new();
        assert!(enumeration.visit_with(provisional.hwnd, |_| {
            super::PointBoundWindowInspection::PassThrough(provisional)
        }));
        let observations = super::finish_point_bound_window_enumeration(enumeration, true, None)
            .expect("a completed exact provisional prefix should reach open desktop");
        assert_eq!(observations, vec![provisional]);

        let hits = vec![provisional_hit(provisional)];
        let stack = stabilize_identical_observations(
            sampled_point,
            &observations,
            hits.clone(),
            &observations,
        );
        assert_eq!(
            stack,
            PlatformWindowHitStack::try_available_open_desktop(sampled_point, hits)
                .expect("the verified desktop underlay should remain explicit")
        );
        assert!(stack.observation().is_some_and(|observation| {
            observation.terminus() == open_gpui::PlatformWindowHitTerminus::OpenDesktop
        }));
    }

    #[test]
    fn window_from_point_verifier_requires_an_exact_pass_through_witness() {
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let geometry = checked_geometry(
            point(DevicePixels(5), DevicePixels(5)),
            size(DevicePixels(80), DevicePixels(80)),
            1.0,
        );
        let window = AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(206_u64)));
        let provisional_hwnd = HWND(0xc6usize as *mut core::ffi::c_void);
        let provisional =
            provisional_observation(provisional_hwnd, 26, window, 36, coverage, geometry);
        let terminal = covering_observation(HWND(0xc7usize as *mut core::ffi::c_void), coverage);
        let observations = [provisional, terminal];

        assert!(super::window_from_point_suffix_agrees(
            provisional_hwnd,
            &observations,
            &observations,
        ));
        assert!(super::window_from_point_suffix_agrees(
            terminal.hwnd,
            &observations,
            &[terminal],
        ));

        let stale_generation =
            provisional_observation(provisional_hwnd, 26, window, 37, coverage, geometry);
        assert!(!super::window_from_point_suffix_agrees(
            provisional_hwnd,
            &observations,
            &[stale_generation, terminal],
        ));
        assert!(!super::window_from_point_suffix_agrees(
            HWND(0xc8usize as *mut core::ffi::c_void),
            &observations,
            &observations,
        ));
    }

    #[test]
    fn native_input_target_rejects_ordinary_prefixes_but_preserves_provisional_authority() {
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let overlay = covering_observation(HWND(0xd1usize as *mut core::ffi::c_void), coverage);
        let terminal = covering_observation(HWND(0xd2usize as *mut core::ffi::c_void), coverage);
        assert!(matches!(
            super::classify_point_bound_window_candidate(overlay, Some(terminal.hwnd), None),
            super::PointBoundWindowInspection::Terminal(observation)
                if observation == overlay
        ));
        let mut mismatch = super::PointBoundWindowEnumeration::new();
        assert!(!mismatch.visit_with(overlay.hwnd, |_| {
            super::PointBoundWindowInspection::Terminal(overlay)
        }));
        assert!(
            super::finish_point_bound_window_enumeration(mismatch, false, Some(terminal.hwnd),)
                .is_none(),
            "WindowFromPoint disagreement with an ordinary covering prefix must fail closed"
        );
        assert!(matches!(
            super::classify_point_bound_window_candidate(terminal, Some(terminal.hwnd), None),
            super::PointBoundWindowInspection::Terminal(observation)
                if observation == terminal
        ));

        let geometry = checked_geometry(
            point(DevicePixels(5), DevicePixels(5)),
            size(DevicePixels(80), DevicePixels(80)),
            1.0,
        );
        let window = AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(207_u64)));
        let provisional = provisional_observation(
            HWND(0xd3usize as *mut core::ffi::c_void),
            27,
            window,
            37,
            coverage,
            geometry,
        );
        let super::NativeCoveringWindowRole::ProvisionalPassThrough(provisional_authority) =
            provisional.role
        else {
            panic!("test observation should carry provisional authority")
        };
        assert!(matches!(
            super::classify_point_bound_window_candidate(
                provisional,
                Some(terminal.hwnd),
                Some(super::RegisteredPointHitInspection::ProvisionalPassThrough(
                    provisional_authority,
                )),
            ),
            super::PointBoundWindowInspection::PassThrough(observation)
                if observation.hwnd == provisional.hwnd
        ));
        assert!(matches!(
            super::classify_point_bound_window_candidate(
                provisional,
                Some(provisional.hwnd),
                Some(super::RegisteredPointHitInspection::ProvisionalPassThrough(
                    provisional_authority,
                )),
            ),
            super::PointBoundWindowInspection::Failed
        ));
    }

    #[test]
    fn native_hovered_window_preserves_unavailable_no_window_and_terminal_states() {
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let terminal = covering_observation(HWND(0xc9usize as *mut core::ffi::c_void), coverage);

        assert_eq!(
            super::classify_native_hovered_window(None),
            super::NativeHoveredWindow::Unavailable
        );
        assert_eq!(
            super::classify_native_hovered_window(Some(&[])),
            super::NativeHoveredWindow::NoWindow
        );
        assert_eq!(
            super::classify_native_hovered_window(Some(&[terminal])),
            super::NativeHoveredWindow::Window(terminal.hwnd)
        );
    }

    #[test]
    fn hit_stack_stabilization_rejects_provisional_authority_and_geometry_drift() {
        let sampled_point = point(DevicePixels(20), DevicePixels(20));
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let moved_coverage = checked_coverage(
            point(DevicePixels(1), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let geometry = checked_geometry(
            point(DevicePixels(5), DevicePixels(5)),
            size(DevicePixels(80), DevicePixels(80)),
            1.0,
        );
        let changed_geometry = checked_geometry(
            point(DevicePixels(6), DevicePixels(5)),
            size(DevicePixels(80), DevicePixels(80)),
            1.0,
        );
        let window = AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(211_u64)));
        let other_window =
            AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(212_u64)));
        let hwnd = HWND(0xd1usize as *mut core::ffi::c_void);
        let terminal = covering_observation(HWND(0xd2usize as *mut core::ffi::c_void), coverage);
        let base = provisional_observation(hwnd, 41, window, 51, coverage, geometry);
        let base_observations = vec![base, terminal];
        let terminal_hit = PlatformWindowHit::OpaqueBarrier { coverage };
        let base_hits = vec![provisional_hit(base), terminal_hit];
        let drifted = [
            provisional_observation(hwnd, 41, window, 52, coverage, geometry),
            provisional_observation(hwnd, 42, window, 51, coverage, geometry),
            provisional_observation(hwnd, 41, other_window, 51, coverage, geometry),
            provisional_observation(hwnd, 41, window, 51, moved_coverage, geometry),
            provisional_observation(hwnd, 41, window, 51, coverage, changed_geometry),
        ];

        for changed in drifted {
            let changed_observations = vec![changed, terminal];
            let changed_hits = vec![provisional_hit(changed), terminal_hit];
            assert_eq!(
                super::stabilized_window_hit_stack(
                    sampled_point,
                    &base_observations,
                    &base_hits,
                    &changed_observations,
                    &changed_hits,
                    &changed_observations,
                    changed_hits.clone(),
                    &changed_observations,
                ),
                PlatformWindowHitStack::Unavailable
            );
        }

        let verifier_generation_drift =
            provisional_observation(hwnd, 41, window, 52, coverage, geometry);
        assert_eq!(
            super::stabilized_window_hit_stack(
                sampled_point,
                &base_observations,
                &base_hits,
                &base_observations,
                &base_hits,
                &base_observations,
                base_hits.clone(),
                &[verifier_generation_drift, terminal],
            ),
            PlatformWindowHitStack::Unavailable
        );
    }

    #[test]
    fn hit_stack_stabilization_rejects_no_input_generation_drift() {
        let sampled_point = point(DevicePixels(20), DevicePixels(20));
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let window = AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(213_u64)));
        let hwnd = HWND(0xd3usize as *mut core::ffi::c_void);
        let terminal = covering_observation(HWND(0xd4usize as *mut core::ffi::c_void), coverage);
        let base = no_input_observation(hwnd, 43, window, 53, coverage);
        let changed = no_input_observation(hwnd, 43, window, 54, coverage);
        let base_hits = vec![
            no_input_hit(base),
            PlatformWindowHit::OpaqueBarrier { coverage },
        ];
        let changed_hits = vec![
            no_input_hit(changed),
            PlatformWindowHit::OpaqueBarrier { coverage },
        ];

        assert_eq!(
            super::stabilized_window_hit_stack(
                sampled_point,
                &[base, terminal],
                &base_hits,
                &[changed, terminal],
                &changed_hits,
                &[changed, terminal],
                changed_hits.clone(),
                &[changed, terminal],
            ),
            PlatformWindowHitStack::Unavailable
        );
    }

    #[test]
    fn unproven_provisional_appearance_remains_a_terminal_barrier() {
        let sampled_point = point(DevicePixels(20), DevicePixels(20));
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let geometry = checked_geometry(
            point(DevicePixels(5), DevicePixels(5)),
            size(DevicePixels(80), DevicePixels(80)),
            1.0,
        );
        let hwnd = HWND(0xe1usize as *mut core::ffi::c_void);
        let terminal = covering_observation(hwnd, coverage);
        let mut enumeration = super::PointBoundWindowEnumeration::new();
        assert!(!enumeration.visit_with(hwnd, |_| {
            super::PointBoundWindowInspection::Terminal(terminal)
        }));
        assert_eq!(
            enumeration
                .finish(false)
                .expect("an unproven window must terminate the observation"),
            vec![terminal]
        );

        let fabricated_pass_through = PlatformWindowHit::ProvisionalPassThrough {
            window: AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(221_u64))),
            session_generation: 61,
            coverage,
            geometry,
        };
        assert_eq!(
            stabilize_identical_observations(
                sampled_point,
                &[terminal],
                vec![fabricated_pass_through],
                &[terminal],
            ),
            PlatformWindowHitStack::Unavailable,
            "a provisional visual shape without an exact current session cannot become pass-through"
        );
    }

    #[test]
    fn hit_stack_stabilization_accepts_only_a_complete_stable_terminal_prefix() {
        let first_hwnd = HWND(0x91usize as *mut core::ffi::c_void);
        let second_hwnd = HWND(0x92usize as *mut core::ffi::c_void);
        let sampled_point = point(DevicePixels(0), DevicePixels(30));
        let first_coverage = checked_coverage(
            point(DevicePixels(-20), DevicePixels(10)),
            size(DevicePixels(100), DevicePixels(80)),
        );
        let second_coverage = checked_coverage(
            point(DevicePixels(-10), DevicePixels(20)),
            size(DevicePixels(90), DevicePixels(70)),
        );
        let first = covering_observation(first_hwnd, first_coverage);
        let second = covering_observation(second_hwnd, second_coverage);
        let observations = vec![first];
        let hits = vec![PlatformWindowHit::OpaqueBarrier {
            coverage: first_coverage,
        }];

        assert_eq!(
            stabilize_identical_observations(
                sampled_point,
                &observations,
                hits.clone(),
                &observations,
            ),
            PlatformWindowHitStack::try_available(sampled_point, hits.clone())
                .expect("the stable point-bound hit should be available")
        );

        let observations_past_terminal = vec![first, second];
        let hits_past_terminal = vec![
            PlatformWindowHit::OpaqueBarrier {
                coverage: first_coverage,
            },
            PlatformWindowHit::OpaqueBarrier {
                coverage: second_coverage,
            },
        ];
        assert_eq!(
            stabilize_identical_observations(
                sampled_point,
                &observations_past_terminal,
                hits_past_terminal,
                &observations_past_terminal,
            ),
            PlatformWindowHitStack::Unavailable
        );

        let incomplete_hits = Vec::new();
        assert_eq!(
            super::stabilized_window_hit_stack(
                sampled_point,
                &observations,
                &hits,
                &observations,
                &incomplete_hits,
                &observations,
                hits.clone(),
                &observations,
            ),
            PlatformWindowHitStack::Unavailable
        );
    }

    #[test]
    fn hit_stack_stabilization_rejects_native_geometry_and_classification_drift() {
        let hwnd = HWND(0xa1usize as *mut core::ffi::c_void);
        let sampled_point = point(DevicePixels(20), DevicePixels(20));
        let coverage = checked_coverage(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let moved_coverage = checked_coverage(
            point(DevicePixels(1), DevicePixels(0)),
            size(DevicePixels(100), DevicePixels(100)),
        );
        let observations = vec![covering_observation(hwnd, coverage)];
        let moved_observations = vec![covering_observation(hwnd, moved_coverage)];
        let window =
            open_gpui::AnyWindowHandle::from(WindowHandle::<Empty>::new(WindowId::from(101_u64)));
        let geometry = checked_geometry(
            point(DevicePixels(5), DevicePixels(5)),
            size(DevicePixels(80), DevicePixels(80)),
            1.0,
        );
        let changed_geometry = checked_geometry(
            point(DevicePixels(5), DevicePixels(5)),
            size(DevicePixels(80), DevicePixels(80)),
            1.25,
        );
        let registered_hit = PlatformWindowHit::RegisteredApplication {
            window,
            coverage,
            geometry,
        };
        let changed_geometry_hit = PlatformWindowHit::RegisteredApplication {
            window,
            coverage,
            geometry: changed_geometry,
        };

        assert_eq!(
            super::stabilized_window_hit_stack(
                sampled_point,
                &observations,
                &[registered_hit],
                &moved_observations,
                &[PlatformWindowHit::RegisteredApplication {
                    window,
                    coverage: moved_coverage,
                    geometry,
                }],
                &moved_observations,
                vec![PlatformWindowHit::RegisteredApplication {
                    window,
                    coverage: moved_coverage,
                    geometry,
                }],
                &moved_observations,
            ),
            PlatformWindowHitStack::Unavailable
        );

        assert_eq!(
            super::stabilized_window_hit_stack(
                sampled_point,
                &observations,
                &[registered_hit],
                &observations,
                &[changed_geometry_hit],
                &observations,
                vec![changed_geometry_hit],
                &observations,
            ),
            PlatformWindowHitStack::Unavailable
        );
    }

    #[test]
    fn hit_stack_stabilization_rejects_verifier_disagreement_and_point_mismatch() {
        let observed_hwnd = HWND(0xb1usize as *mut core::ffi::c_void);
        let other_hwnd = HWND(0xb2usize as *mut core::ffi::c_void);
        let coverage = checked_coverage(
            point(DevicePixels(10), DevicePixels(10)),
            size(DevicePixels(20), DevicePixels(20)),
        );
        let observations = vec![covering_observation(observed_hwnd, coverage)];
        let other_observations = vec![covering_observation(other_hwnd, coverage)];
        let hits = vec![PlatformWindowHit::OpaqueBarrier { coverage }];

        assert_eq!(
            stabilize_identical_observations(
                point(DevicePixels(15), DevicePixels(15)),
                &observations,
                hits.clone(),
                &other_observations,
            ),
            PlatformWindowHitStack::Unavailable
        );
        assert_eq!(
            stabilize_identical_observations(
                point(DevicePixels(15), DevicePixels(15)),
                &observations,
                hits.clone(),
                &[],
            ),
            PlatformWindowHitStack::Unavailable
        );
        assert_eq!(
            stabilize_identical_observations(
                point(DevicePixels(9), DevicePixels(15)),
                &observations,
                hits,
                &observations,
            ),
            PlatformWindowHitStack::Unavailable
        );
        assert_eq!(
            stabilize_identical_observations(
                point(DevicePixels(100), DevicePixels(100)),
                &[],
                Vec::new(),
                &[],
            ),
            PlatformWindowHitStack::try_available_open_desktop(
                point(DevicePixels(100), DevicePixels(100)),
                Vec::new(),
            )
            .expect("verified open desktop space should be available")
        );
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
                    provisional_presentation: WindowCreationSupport::Supported,
                    initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
                },
                mutations: PlatformWindowMutationCapabilities {
                    position: WindowMutationSupport::CreationOnly,
                    physical_placement: WindowMutationSupport::Live,
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
                activation: PlatformWindowActivationSupport::Observed,
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
        let registered_window = *platform
            .raw_window_handles
            .read()
            .last()
            .expect("native test window handle should be registered");
        let native_window = registered_window.as_raw();
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

        let mut first_no_input_generation = None;
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
            let registered_inspection =
                super::inspect_registered_window_for_point(&platform, registered_window);
            if let super::RegisteredPointHitInspection::NoInputPassThrough(observation) =
                registered_inspection
            {
                first_no_input_generation = Some(observation.pointer_input_generation);
                let current = app.update_for_test(|cx| {
                    window
                        .update(cx, |_, window, _| {
                            window.is_current_pointer_input_observation(
                                false,
                                observation.pointer_input_generation,
                            )
                        })
                        .expect("native test window should remain open")
                });
                assert!(current);
            }
            assert!(
                matches!(
                    (accepts_pointer_input, registered_inspection),
                    (
                        false,
                        super::RegisteredPointHitInspection::NoInputPassThrough(_)
                    ) | (true, super::RegisteredPointHitInspection::Terminal)
                ),
                "the stable point sampler must match the committed pointer-input fact"
            );

            let mut native_rect = RECT::default();
            unsafe { GetWindowRect(native_window, &mut native_rect) }
                .expect("the native test window bounds should remain readable");
            let sampled_point = point(
                DevicePixels(native_rect.left + (native_rect.right - native_rect.left) / 2),
                DevicePixels(native_rect.top + (native_rect.bottom - native_rect.top) / 2),
            );
            let point_inspection = super::inspect_point_bound_window(
                &platform,
                sampled_point,
                accepts_pointer_input.then_some(native_window),
                native_window,
                &[registered_window],
            );
            assert!(
                matches!(
                    (accepts_pointer_input, point_inspection),
                    (false, super::PointBoundWindowInspection::PassThrough(_))
                        | (true, super::PointBoundWindowInspection::Terminal(_))
                ),
                "a committed no-input HWND must remain a pass-through point-stack fact"
            );
        }

        let first_no_input_generation =
            first_no_input_generation.expect("the no-input mutation must publish a generation");
        let observed = observe_native_mutation(
            &platform,
            &mut app,
            window,
            WindowMutationRequest::PointerInput(false),
        );
        assert_eq!(observed.outcome, WindowMutationOutcome::Exact);
        let super::RegisteredPointHitInspection::NoInputPassThrough(current_no_input) =
            super::inspect_registered_window_for_point(&platform, registered_window)
        else {
            panic!("the second no-input mutation must publish a pass-through observation");
        };
        assert_ne!(
            current_no_input.pointer_input_generation, first_no_input_generation,
            "a false -> true -> false ABA must advance the pointer-input authority generation"
        );
        let (stale_rejected, current_accepted) = app.update_for_test(|cx| {
            window
                .update(cx, |_, window, _| {
                    (
                        window
                            .is_current_pointer_input_observation(false, first_no_input_generation),
                        window.is_current_pointer_input_observation(
                            false,
                            current_no_input.pointer_input_generation,
                        ),
                    )
                })
                .expect("native test window should remain open")
        });
        assert!(!stale_rejected);
        assert!(current_accepted);

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

        let _ = app.update_for_test(|cx| {
            let _ = window
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

    #[test]
    fn ordinary_window_merges_physical_placement_before_first_presentation() {
        let platform = Rc::new(super::WindowsPlatform::new(false).unwrap());
        let mut app = Application::with_platform(platform.clone());
        let (window, native_window, request, dispatch) = app.update_for_test(|cx| {
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::centered(size(px(320.0), px(220.0)), cx)),
                        focus_on_appearing: false,
                        show: true,
                        ..WindowOptions::default()
                    },
                    |_, cx| cx.new(|_| Empty),
                )
                .expect("ordinary native test window should open");
            let native_window = platform
                .raw_window_handles
                .read()
                .last()
                .expect("ordinary native test window handle should be registered")
                .as_raw();
            assert!(!unsafe { IsWindowVisible(native_window).as_bool() });

            let mut client_rect = RECT::default();
            unsafe { GetClientRect(native_window, &mut client_rect) }
                .expect("initial native client bounds should be readable");
            let mut client_origin = POINT { x: 0, y: 0 };
            unsafe { ClientToScreen(native_window, &mut client_origin) }
                .expect("initial native client origin should be readable");
            let target_bounds = Bounds {
                origin: point(
                    DevicePixels(client_origin.x + 8),
                    DevicePixels(client_origin.y + 8),
                ),
                size: size(
                    DevicePixels(client_rect.right - client_rect.left + 32),
                    DevicePixels(client_rect.bottom - client_rect.top + 24),
                ),
            };
            let anchor_point = point(
                DevicePixels(client_origin.x + 16),
                DevicePixels(client_origin.y + 16),
            );
            let target_display = platform
                .inner
                .exact_display_topology_snapshot()
                .expect("ordinary placement should observe an exact display topology")
                .physical_observation_at(anchor_point)
                .expect("ordinary physical placement should resolve its target display");
            let request = WindowPhysicalPlacementRequest::try_new_for_display(
                target_bounds,
                anchor_point,
                target_display,
            )
            .expect("ordinary physical placement should bind its target display");
            let dispatch = window
                .update(cx, |_, window, _| {
                    window
                        .request_window_mutation(WindowMutationRequest::PhysicalPlacement(request))
                })
                .expect("ordinary native test window should remain open");
            (window, native_window, request, dispatch)
        });
        let ticket = match dispatch {
            WindowMutationDispatch::Queued(ticket) => ticket,
            other => panic!("expected deferred physical placement, got {other:?}"),
        };

        let observation = (0..16)
            .find_map(|_| {
                platform.inner.run_foreground_task();
                ticket.observation()
            })
            .expect("first presentation should settle deferred physical placement");
        assert_eq!(observation.outcome, WindowMutationOutcome::Exact);
        assert!(
            observation
                .facts
                .physical_geometry
                .is_some_and(|geometry| request.matches_geometry(geometry))
        );
        assert!(
            unsafe { IsWindowVisible(native_window).as_bool() },
            "the first presentation should show the physically placed window"
        );

        let window = open_gpui::AnyWindowHandle::from(window);
        app.update_for_test(|cx| {
            window
                .update(cx, |_, window, cx| window.remove_window(cx))
                .expect("ordinary native test window should close");
        });
        platform.inner.run_foreground_task();
    }
}
