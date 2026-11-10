use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    env,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
#[cfg(any(feature = "wayland", feature = "x11"))]
use std::{ffi::OsString, fs::File, os::fd::AsFd, time::Duration};
#[cfg(feature = "wayland")]
use std::{io::Read as _, os::fd::AsRawFd};

use anyhow::{Context as _, anyhow};
#[cfg(any(feature = "wayland", feature = "x11"))]
use ashpd::{
    desktop::{
        Request,
        inhibit::{InhibitFlags, InhibitOptions, InhibitProxy},
    },
    enumflags2::BitFlags,
};
#[cfg(any(feature = "wayland", feature = "x11"))]
use calloop::channel::Sender;
use calloop::{EventLoop, LoopHandle, LoopSignal, RegistrationToken};
use futures::channel::oneshot;
use open_gpui_util::ResultExt as _;
use open_gpui_util::command::new_std_command;
#[cfg(any(feature = "wayland", feature = "x11"))]
use xkbcommon::xkb::{self, Keycode, Keysym, State};

use crate::linux::{
    Backend, DisplayConnection, HeadlessConnection, LinuxDispatcher, PriorityQueueCalloopReceiver,
    select_backend,
};
use open_gpui::{
    Action, ActivityGuard, AnyWindowHandle, BackgroundExecutor, ClipboardItem, DisplayId,
    ForegroundExecutor, GraphicalEnvironment, Keymap, Menu, MenuItem, MouseButton,
    NativeAppNotificationHandler, OwnedMenu, PathPromptOptions, Platform, PlatformDisplay,
    PlatformFocusedWindow, PlatformHoveredWindow, PlatformKeyboardLayout, PlatformKeyboardMapper,
    PlatformTextSystem, PlatformViewportCapabilities, PlatformWindow, PlatformWindowCapabilities,
    PowerMode, Result, RunnableVariant, Task, ThermalState, WindowAppearance, WindowButtonLayout,
    WindowKind, WindowParams, WindowingModes, WindowingRequest,
};
#[cfg(any(feature = "wayland", feature = "x11"))]
use open_gpui::{CursorStyle, Pixels, Point, px};

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) const SCROLL_LINES: f32 = 3.0;

// Values match the defaults on GTK.
// Taken from https://github.com/GNOME/gtk/blob/main/gtk/gtksettings.c#L320
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(400);
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) const DOUBLE_CLICK_DISTANCE: Pixels = px(5.0);
pub(crate) const KEYRING_LABEL: &str = "open-gpui-github-account";

type ForegroundSource =
    calloop::Dispatcher<'static, PriorityQueueCalloopReceiver<RunnableVariant>, ()>;

#[cfg(any(feature = "wayland", feature = "x11"))]
const FILE_PICKER_PORTAL_MISSING: &str =
    "Couldn't open file picker due to missing xdg-desktop-portal implementation.";

#[derive(Default)]
pub(crate) struct PlatformHandlers {
    pub(crate) open_urls: Option<Box<dyn FnMut(Vec<String>)>>,
    pub(crate) quit: Option<Box<dyn FnMut() -> bool>>,
    pub(crate) reopen: Option<Box<dyn FnMut()>>,
    pub(crate) app_menu_action: Option<Box<dyn FnMut(&dyn Action)>>,
    pub(crate) will_open_app_menu: Option<Box<dyn FnMut()>>,
    pub(crate) validate_app_menu_command: Option<Box<dyn FnMut(&dyn Action) -> bool>>,
    pub(crate) keyboard_layout_change: Option<Box<dyn FnMut()>>,
    pub(crate) displays_changed: Option<Box<dyn FnMut()>>,
    pub(crate) system_sleep: Option<NativeAppNotificationHandler>,
    pub(crate) system_wake: Option<NativeAppNotificationHandler>,
    pub(crate) power_mode_change: Option<NativeAppNotificationHandler>,
}

impl PlatformHandlers {
    pub(crate) fn system_power_callback(
        &self,
        event: SystemPowerEvent,
    ) -> Option<NativeAppNotificationHandler> {
        match event {
            SystemPowerEvent::Sleep => self.system_sleep.clone(),
            SystemPowerEvent::Wake => self.system_wake.clone(),
            SystemPowerEvent::ModeChanged(_) => self.power_mode_change.clone(),
        }
    }
}

/// A system power change forwarded from D-Bus to the client's event loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(all(target_os = "linux", any(feature = "wayland", feature = "x11"))),
    allow(dead_code)
)]
pub(crate) enum SystemPowerEvent {
    Sleep,
    Wake,
    ModeChanged(PowerMode),
}

pub(crate) struct LinuxCommon {
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
    dispatcher: Arc<LinuxDispatcher>,
    pub(crate) text_system: Arc<dyn PlatformTextSystem>,
    pub(crate) appearance: WindowAppearance,
    pub(crate) auto_hide_scrollbars: bool,
    pub(crate) button_layout: WindowButtonLayout,
    pub(crate) callbacks: PlatformHandlers,
    pub(crate) signal: LoopSignal,
    pub(crate) menus: Vec<OwnedMenu>,
    app_name: Option<String>,
    system_notifications: crate::linux::system_notifications::SystemNotificationState,
    #[cfg(any(feature = "wayland", feature = "x11"))]
    power_sender: Sender<SystemPowerEvent>,
    #[cfg(all(target_os = "linux", any(feature = "wayland", feature = "x11")))]
    system_power_listener: Option<Task<()>>,
    power_mode: PowerMode,
    #[cfg(any(feature = "wayland", feature = "x11"))]
    power_mode_listener: Option<Task<()>>,
    power_observation_stopped: bool,
    pub(crate) after_runnable: Option<Rc<dyn Fn()>>,
    foreground_idles: VecDeque<calloop::Idle<'static>>,
}

impl LinuxCommon {
    pub fn new(
        signal: LoopSignal,
    ) -> (
        Self,
        PriorityQueueCalloopReceiver<RunnableVariant>,
        calloop::channel::Channel<SystemPowerEvent>,
    ) {
        let (main_sender, main_receiver) = PriorityQueueCalloopReceiver::new();
        let (power_sender, power_receiver) = calloop::channel::channel();
        #[cfg(not(any(feature = "wayland", feature = "x11")))]
        let _ = power_sender;

        #[cfg(any(feature = "wayland", feature = "x11"))]
        let text_system = Arc::new(crate::linux::CosmicTextSystem::new("IBM Plex Sans"));
        #[cfg(not(any(feature = "wayland", feature = "x11")))]
        let text_system = Arc::new(open_gpui::NoopTextSystem::new());

        let callbacks = PlatformHandlers::default();

        let dispatcher = Arc::new(LinuxDispatcher::new(main_sender));

        let background_executor = BackgroundExecutor::new(dispatcher.clone());

        let common = LinuxCommon {
            background_executor,
            foreground_executor: ForegroundExecutor::new(dispatcher.clone()),
            dispatcher,
            text_system,
            appearance: WindowAppearance::Light,
            auto_hide_scrollbars: false,
            button_layout: WindowButtonLayout::linux_default(),
            callbacks,
            signal,
            menus: Vec::new(),
            app_name: None,
            system_notifications: crate::linux::system_notifications::SystemNotificationState::new(
            ),
            #[cfg(any(feature = "wayland", feature = "x11"))]
            power_sender,
            #[cfg(all(target_os = "linux", any(feature = "wayland", feature = "x11")))]
            system_power_listener: None,
            power_mode: PowerMode::Unknown,
            #[cfg(any(feature = "wayland", feature = "x11"))]
            power_mode_listener: None,
            power_observation_stopped: false,
            after_runnable: None,
            foreground_idles: VecDeque::new(),
        };

        (common, main_receiver, power_receiver)
    }

    pub(crate) fn start_power_listener(&mut self) {
        if self.power_observation_stopped {
            return;
        }

        #[cfg(all(target_os = "linux", any(feature = "wayland", feature = "x11")))]
        if self.system_power_listener.is_none() {
            let power_sender = self.power_sender.clone();
            self.system_power_listener = Some(self.background_executor.spawn(async move {
                if let Err(error) = listen_for_system_power_events(power_sender).await {
                    log::debug!("failed to listen for system sleep/wake events: {error:?}");
                }
            }));
        }
    }

    #[cfg(any(feature = "wayland", feature = "x11"))]
    fn start_power_mode_listener(&mut self) {
        if self.power_observation_stopped || self.power_mode_listener.is_some() {
            return;
        }

        let sender = self.power_sender.clone();
        self.power_mode_listener = Some(self.background_executor.spawn(async move {
            let result = async {
                let connection = ashpd::zbus::Connection::system().await?;
                crate::linux::power::monitor_power_mode(&connection, |mode| {
                    sender.send(SystemPowerEvent::ModeChanged(mode))?;
                    Ok(())
                })
                .await
            }
            .await;
            if let Err(error) = result {
                sender
                    .send(SystemPowerEvent::ModeChanged(PowerMode::Unknown))
                    .ok();
                log::debug!("failed to monitor system power mode: {error:?}");
            }
        }));
    }

    /// Registers the foreground executor and power listener on a loop shared with display clients.
    ///
    /// Foreground runnables run as idle callbacks, after the loop has dispatched all pending events.
    pub(crate) fn register_sources(
        common: &Rc<RefCell<Self>>,
        loop_handle: &calloop::LoopHandle<'static, ()>,
        main_receiver: PriorityQueueCalloopReceiver<RunnableVariant>,
        power_receiver: calloop::channel::Channel<SystemPowerEvent>,
    ) -> anyhow::Result<(ForegroundSource, Vec<RegistrationToken>)> {
        let foreground = calloop::Dispatcher::new(main_receiver, {
            let loop_handle = loop_handle.clone();
            let common = common.clone();
            move |event, _, _| {
                if let calloop::channel::Event::Msg(runnable) = event {
                    let idle_common = common.clone();
                    let idle = loop_handle.insert_idle(move |_| {
                        idle_common.borrow_mut().foreground_idles.pop_front();
                        let location = runnable.metadata().location;
                        let spawned = runnable.metadata().spawned;
                        let _task_timing = open_gpui::profiler::task_timing(spawned, location);
                        runnable.run();

                        let after_runnable = idle_common.borrow().after_runnable.clone();
                        if let Some(after_runnable) = after_runnable {
                            after_runnable();
                        }
                    });
                    common.borrow_mut().foreground_idles.push_back(idle);
                }
            }
        });
        let foreground_token = loop_handle
            .register_dispatcher(foreground.clone())
            .map_err(|error| anyhow!("failed to register foreground executor: {error}"))?;
        let power = loop_handle
            .insert_source(power_receiver, {
                let common = common.clone();
                move |event, _, _| {
                    if let calloop::channel::Event::Msg(event) = event {
                        let callback = common.borrow_mut().handle_system_power_event(event);
                        if let Some(callback) = callback {
                            callback();
                        }
                    }
                }
            })
            .map_err(|error| {
                loop_handle.remove(foreground_token);
                anyhow!("failed to register power listener: {error}")
            })?;
        Ok((foreground, vec![foreground_token, power]))
    }

    pub(crate) fn handle_system_power_event(
        &mut self,
        event: SystemPowerEvent,
    ) -> Option<NativeAppNotificationHandler> {
        if self.power_observation_stopped {
            return None;
        }
        if let SystemPowerEvent::ModeChanged(mode) = event {
            if self.power_mode == mode {
                return None;
            }
            self.power_mode = mode;
        }
        self.callbacks.system_power_callback(event)
    }

    fn stop_power_listeners(&mut self) {
        self.power_observation_stopped = true;
        self.power_mode = PowerMode::Unknown;
        #[cfg(all(target_os = "linux", any(feature = "wayland", feature = "x11")))]
        self.system_power_listener.take();
        #[cfg(any(feature = "wayland", feature = "x11"))]
        self.power_mode_listener.take();
        self.callbacks.system_sleep.take();
        self.callbacks.system_wake.take();
        self.callbacks.power_mode_change.take();
    }
}

#[cfg(all(target_os = "linux", any(feature = "wayland", feature = "x11")))]
async fn listen_for_system_power_events(
    power_sender: Sender<SystemPowerEvent>,
) -> anyhow::Result<()> {
    use futures::StreamExt as _;

    let connection = ashpd::zbus::Connection::system().await?;
    let proxy = ashpd::zbus::Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await?;
    let mut sleep_events = proxy.receive_signal("PrepareForSleep").await?;

    while let Some(message) = sleep_events.next().await {
        let event = if message.body().deserialize::<bool>()? {
            SystemPowerEvent::Sleep
        } else {
            SystemPowerEvent::Wake
        };
        power_sender.send(event)?;
    }

    Ok(())
}

/// GPUI's platform for Linux and FreeBSD.
///
/// One event loop serves the whole process. The foreground executor and power listener are
/// registered on it once, and the current [`DisplayConnection`] adds and removes its own sources
/// as the app switches between the display modes it allows.
pub(crate) struct LinuxPlatform {
    common: Rc<RefCell<LinuxCommon>>,
    /// The event loop, until `run` takes it.
    event_loop: RefCell<Option<EventLoop<'static, ()>>>,
    loop_handle: LoopHandle<'static, ()>,
    foreground_source: ForegroundSource,
    common_sources: RefCell<Vec<RegistrationToken>>,
    connection: RefCell<DisplayConnection>,
    allowed_modes: WindowingModes,
    /// The mode to start in, applied when `run` starts. Windowed in the process's own
    /// environment when unset.
    initial_windowing: RefCell<Option<WindowingRequest>>,
    /// Taken by the connection made when `run` starts.
    startup_environment: RefCell<Option<StartupEnvironment>>,
    /// A requested switch waiting for the event loop to apply it.
    pending_mode: RefCell<Option<PendingWindowing>>,
    transition_in_progress: Cell<bool>,
    started: Cell<bool>,
    stopped: Cell<bool>,
}

struct PendingWindowing {
    request: WindowingRequest,
    validate: Box<dyn Fn() -> anyhow::Result<()>>,
    waiter: Option<oneshot::Sender<anyhow::Result<()>>>,
}

impl PendingWindowing {
    fn finish(mut self, result: anyhow::Result<()>) {
        if let Some(waiter) = self.waiter.take() {
            waiter.send(result).ok();
        }
    }
}

impl Drop for PendingWindowing {
    fn drop(&mut self) {
        if let Some(waiter) = self.waiter.take() {
            waiter
                .send(Err(anyhow!(
                    "display mode transition was canceled before completion"
                )))
                .ok();
        }
    }
}

struct TransitionGuard<'a> {
    in_progress: &'a Cell<bool>,
    admitted: bool,
}

impl Drop for TransitionGuard<'_> {
    fn drop(&mut self) {
        if !self.admitted {
            self.in_progress.set(false);
        }
    }
}

struct RuntimeShutdown<'a>(&'a LinuxPlatform);

impl Drop for RuntimeShutdown<'_> {
    fn drop(&mut self) {
        self.0.finish_runtime();
    }
}

/// What a Wayland connection made at startup may use from this process's environment.
///
/// The token is captured once without mutating the process environment. Socket ownership must be
/// supplied explicitly by the startup owner; an environment string cannot establish that a running
/// Host has transferred a descriptor. Both resources are meaningful only for the first connection.
#[derive(Default)]
pub(crate) struct StartupEnvironment {
    #[cfg(feature = "wayland")]
    activation_token: Option<String>,
    /// The socket `WAYLAND_SOCKET` names: a connection to the compositor that the process which
    /// started us already opened.
    #[cfg(feature = "wayland")]
    wayland_socket: Option<std::os::unix::net::UnixStream>,
    #[cfg(feature = "wayland")]
    unclaimed_socket: bool,
}

impl StartupEnvironment {
    fn capture() -> Self {
        #[cfg(feature = "wayland")]
        {
            use std::sync::atomic::{AtomicBool, Ordering};
            static CAPTURED: AtomicBool = AtomicBool::new(false);
            if !CAPTURED.swap(true, Ordering::AcqRel) {
                return Self {
                    activation_token: env::var("XDG_ACTIVATION_TOKEN")
                        .ok()
                        .filter(|token| !token.is_empty()),
                    wayland_socket: None,
                    unclaimed_socket: env::var_os("WAYLAND_SOCKET").is_some(),
                };
            }
        }
        Self {
            #[cfg(feature = "wayland")]
            unclaimed_socket: env::var_os("WAYLAND_SOCKET").is_some(),
            ..Self::default()
        }
    }

    #[cfg(feature = "wayland")]
    pub(crate) fn owned(
        wayland_socket: Option<std::os::unix::net::UnixStream>,
        activation_token: Option<String>,
    ) -> Self {
        if let Some(socket) = &wayland_socket {
            use std::os::fd::AsRawFd as _;
            // SAFETY: The supplied stream exclusively owns this valid descriptor. This changes
            // descriptor flags only; it neither adopts nor closes a descriptor owned by the Host.
            let flags = unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_GETFD) };
            assert!(flags != -1, "failed to read owned Wayland socket flags");
            // SAFETY: The stream remains owned and alive, and FD_CLOEXEC prevents child inheritance.
            assert!(
                unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) }
                    != -1,
                "failed to protect owned Wayland socket from child inheritance"
            );
            socket
                .set_nonblocking(false)
                .expect("failed to configure owned Wayland startup socket");
        }
        Self {
            activation_token: activation_token.filter(|token| !token.is_empty()),
            wayland_socket,
            unclaimed_socket: false,
        }
    }
}

impl LinuxPlatform {
    /// Creates the platform, headless until `run` connects it in its initial mode.
    pub(crate) fn new(allowed_modes: WindowingModes) -> Self {
        Self::new_with_startup(allowed_modes, StartupEnvironment::capture())
    }

    pub(crate) fn new_with_startup(
        allowed_modes: WindowingModes,
        startup: StartupEnvironment,
    ) -> Self {
        let event_loop = EventLoop::try_new().expect("failed to create Linux event loop");
        let (common, main_receiver, power_receiver) = LinuxCommon::new(event_loop.get_signal());
        let common = Rc::new(RefCell::new(common));
        let loop_handle = event_loop.handle();
        let (foreground_source, common_sources) =
            LinuxCommon::register_sources(&common, &loop_handle, main_receiver, power_receiver)
                .expect("failed to register Linux event sources");
        Self {
            common,
            event_loop: RefCell::new(Some(event_loop)),
            loop_handle,
            foreground_source,
            common_sources: RefCell::new(common_sources),
            connection: RefCell::new(DisplayConnection::Headless(HeadlessConnection::new())),
            allowed_modes,
            initial_windowing: RefCell::new(None),
            startup_environment: RefCell::new(Some(startup)),
            pending_mode: RefCell::new(None),
            transition_in_progress: Cell::new(false),
            started: Cell::new(false),
            stopped: Cell::new(false),
        }
    }

    /// Connects in the initial mode, before the app finishes launching.
    ///
    /// # Panics
    ///
    /// Panics if the allowed modes don't allow the initial mode, or if its display server can't
    /// be reached, since the app can't start. A windowed initial mode whose environment names no
    /// allowed display server starts headless when that's allowed.
    fn connect_initially(&self) {
        assert!(!self.started.replace(true), "App is already running");
        let request = self
            .initial_windowing
            .borrow_mut()
            .take()
            .unwrap_or_else(|| WindowingRequest::Windowed(GraphicalEnvironment::detect()));
        // Dropped unused when starting headless, closing any explicitly owned startup socket.
        let startup = self
            .startup_environment
            .borrow_mut()
            .take()
            .unwrap_or_default();
        let backend = match &request {
            WindowingRequest::Headless => None,
            WindowingRequest::Windowed(environment) => {
                let backend = select_backend(self.allowed_modes, environment);
                #[cfg(feature = "wayland")]
                let backend = {
                    assert!(
                        !startup.unclaimed_socket || !matches!(backend, Some(Backend::Wayland)),
                        "WAYLAND_SOCKET requires explicit ownership through linux_platform_with_startup; a running Host's descriptor cannot be adopted from an environment string"
                    );
                    if startup.wayland_socket.is_some() {
                        assert!(
                            self.allowed_modes.contains(WindowingModes::WAYLAND),
                            "the startup Wayland socket requires WindowingModes::WAYLAND"
                        );
                        Some(Backend::Wayland)
                    } else {
                        backend
                    }
                };
                backend.map(|backend| (backend, environment))
            }
        };
        match backend {
            Some((backend, environment)) => {
                let connection = self
                    .connect(backend, environment, startup)
                    .unwrap_or_else(|error| panic!("{error:#}"));
                assert!(
                    !self.connection.borrow().has_windows(),
                    "cannot connect initially while headless windows are open"
                );
                self.replace_connection(connection);
                self.notify_connection_changed();
            }
            None => assert!(
                self.allowed_modes.contains(WindowingModes::HEADLESS),
                "{:?} does not allow starting headless, and no allowed display server was given",
                self.allowed_modes
            ),
        }
    }

    fn with_common<R>(&self, function: impl FnOnce(&mut LinuxCommon) -> R) -> R {
        function(&mut self.common.borrow_mut())
    }

    /// Connects to the display server `backend` in `environment`.
    ///
    /// `startup` is only given for the connection made when the platform is created.
    #[cfg_attr(
        not(feature = "wayland"),
        allow(
            unused_variables,
            reason = "only Wayland uses the startup environment, and without a display backend \
                      `Backend` has no variants"
        )
    )]
    fn connect(
        &self,
        backend: Backend,
        environment: &GraphicalEnvironment,
        startup: StartupEnvironment,
    ) -> anyhow::Result<DisplayConnection> {
        let loop_handle = self.loop_handle.clone();
        let common = self.common.clone();
        match backend {
            #[cfg(feature = "wayland")]
            Backend::Wayland => {
                let activation_token = environment
                    .activation_token
                    .clone()
                    .or(startup.activation_token);
                Ok(DisplayConnection::Wayland(
                    crate::linux::WaylandConnection::attach(
                        loop_handle,
                        common,
                        environment,
                        startup.wayland_socket,
                        activation_token,
                    )?,
                ))
            }
            #[cfg(feature = "x11")]
            Backend::X11 => Ok(DisplayConnection::X11(crate::linux::X11Connection::attach(
                loop_handle,
                common,
                environment,
            )?)),
        }
    }

    /// Installs `connection`, then drops the previous one outside the borrow, since dropping a
    /// connection can call back into the platform.
    fn replace_connection(&self, connection: DisplayConnection) {
        let previous = std::mem::replace(&mut *self.connection.borrow_mut(), connection);
        drop(previous);
        self.connection.borrow().did_attach();
        #[cfg(any(feature = "wayland", feature = "x11"))]
        if !self.connection.borrow().is_headless() {
            let mut common = self.common.borrow_mut();
            if common.callbacks.power_mode_change.is_some() {
                common.start_power_mode_listener();
            }
        }
    }

    fn notify_connection_changed(&self) {
        let mut first_panic = None;
        // The app caches the keyboard layout and the displays, which belong to
        // the connection.
        let callback = self
            .common
            .borrow_mut()
            .callbacks
            .keyboard_layout_change
            .take();
        if let Some(mut callback) = callback {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut callback));
            let mut common = self.common.borrow_mut();
            // The callback may have replaced itself.
            if common.callbacks.keyboard_layout_change.is_none() {
                common.callbacks.keyboard_layout_change = Some(callback);
            }
            drop(common);
            if let Err(panic) = result {
                first_panic = Some(panic);
            }
        }
        if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.notify_displays_changed()
        })) {
            first_panic.get_or_insert(panic);
        }
        let callback = self.common.borrow().callbacks.power_mode_change.clone();
        if let Some(callback) = callback
            && let Err(panic) =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback()))
        {
            first_panic.get_or_insert(panic);
        }
        if let Some(panic) = first_panic {
            std::panic::resume_unwind(panic);
        }
    }

    fn notify_displays_changed(&self) {
        let callback = self.common.borrow_mut().callbacks.displays_changed.take();
        if let Some(mut callback) = callback {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut callback));
            self.common
                .borrow_mut()
                .callbacks
                .displays_changed
                .get_or_insert(callback);
            if let Err(panic) = result {
                std::panic::resume_unwind(panic);
            }
        }
    }

    /// Applies a pending switch.
    ///
    /// Runs between event-loop iterations, so no event source is mid-dispatch when the
    /// connection's sources are added or removed.
    fn apply_pending_mode(&self) {
        let Some(pending) = self.pending_mode.borrow_mut().take() else {
            return;
        };
        let guard = TransitionGuard {
            in_progress: &self.transition_in_progress,
            admitted: false,
        };
        let result = (|| {
            self.validate_mode(&pending.request)?;
            self.validate_no_windows()?;
            (pending.validate)()?;
            self.validate_mode(&pending.request)?;
            self.validate_no_windows()?;
            let connection = match &pending.request {
                WindowingRequest::Headless => {
                    DisplayConnection::Headless(HeadlessConnection::new())
                }
                WindowingRequest::Windowed(environment) => {
                    select_backend(self.allowed_modes, &environment)
                        .context("the environment names no allowed Wayland or X11 display server")
                        .and_then(|backend| {
                            self.connect(backend, &environment, StartupEnvironment::default())
                        })?
                }
            };
            (pending.validate)()?;
            self.validate_mode(&pending.request)?;
            self.validate_no_windows()?;
            self.replace_connection(connection);
            Ok(())
        })();
        let notification = if result.is_ok() {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.notify_connection_changed();
            }))
        } else {
            Ok(())
        };
        pending.finish(result);
        drop(guard);
        if let Err(panic) = notification {
            std::panic::resume_unwind(panic);
        }
    }

    fn validate_no_windows(&self) -> anyhow::Result<()> {
        if self.connection.borrow().has_windows() {
            anyhow::bail!("cannot switch display modes while windows are open or retiring");
        }
        Ok(())
    }

    fn validate_mode(&self, mode: &WindowingRequest) -> anyhow::Result<()> {
        if self.stopped.get() {
            anyhow::bail!("cannot switch display modes after platform shutdown");
        }
        let is_headless = self.connection.borrow().is_headless();
        match mode {
            WindowingRequest::Headless
                if !self.allowed_modes.contains(WindowingModes::HEADLESS) =>
            {
                anyhow::bail!("{:?} does not allow headless mode", self.allowed_modes);
            }
            WindowingRequest::Headless if is_headless => {
                anyhow::bail!("already headless");
            }
            WindowingRequest::Headless => {}
            WindowingRequest::Windowed(_) if !is_headless => {
                anyhow::bail!(
                    "already windowed ({}); switch to headless mode first",
                    self.connection.borrow().compositor_name()
                );
            }
            WindowingRequest::Windowed(environment) => {
                if select_backend(self.allowed_modes, environment).is_none() {
                    anyhow::bail!(
                        "the environment names no display server that {:?} allows",
                        self.allowed_modes
                    );
                }
            }
        }
        Ok(())
    }

    /// Validates a requested switch, then leaves it for the event loop to apply.
    fn request_mode(
        &self,
        mode: WindowingRequest,
        validate: Box<dyn Fn() -> anyhow::Result<()>>,
    ) -> Task<anyhow::Result<()>> {
        if self.transition_in_progress.replace(true) {
            return Task::ready(Err(anyhow!(
                "a display mode transition is already in progress"
            )));
        }
        let mut guard = TransitionGuard {
            in_progress: &self.transition_in_progress,
            admitted: false,
        };
        if let Err(error) = validate()
            .and_then(|()| self.validate_mode(&mode))
            .and_then(|()| self.validate_no_windows())
        {
            return Task::ready(Err(error));
        }
        let (sender, receiver) = oneshot::channel();
        self.pending_mode.borrow_mut().replace(PendingWindowing {
            request: mode,
            validate,
            waiter: Some(sender),
        });
        guard.admitted = true;
        // The wakeup persists until the loop next polls, so a request made before `run` starts is
        // applied after its first iteration.
        self.common.borrow().signal.wakeup();
        self.common.borrow().background_executor.spawn(async move {
            receiver
                .await
                .map_err(|_| anyhow!("display mode transition was canceled"))
                .and_then(|result| result)
        })
    }

    fn stop_runtime(&self) {
        self.stopped.set(true);
        let pending = self.pending_mode.borrow_mut().take();
        if let Some(pending) = pending {
            pending.finish(Err(anyhow!(
                "display mode transition refused during platform shutdown"
            )));
            self.transition_in_progress.set(false);
        }
        self.with_common(|common| {
            common.stop_power_listeners();
            common.signal.stop();
            common.signal.wakeup();
        });
    }

    fn finish_runtime(&self) {
        let shutdown_timers = self.started.get() || self.stopped.get();
        self.stop_runtime();
        let runnables = self.foreground_source.as_source_mut().close();
        for token in self.common_sources.take() {
            self.loop_handle.remove(token);
        }
        let idles = std::mem::take(&mut self.common.borrow_mut().foreground_idles);
        drop(runnables);
        for idle in idles {
            idle.cancel();
        }
        if shutdown_timers {
            let dispatcher = self.common.borrow().dispatcher.clone();
            dispatcher.shutdown_timers();
        }
        if !self.connection.borrow().has_windows() {
            self.replace_connection(DisplayConnection::Headless(HeadlessConnection::new()));
        }
    }
}

impl Drop for LinuxPlatform {
    fn drop(&mut self) {
        self.finish_runtime();
        self.replace_connection(DisplayConnection::Headless(HeadlessConnection::new()));
        #[cfg(feature = "x11")]
        crate::linux::wait_for_clipboard_handovers();
    }
}

fn restart_command(
    app_pid: &str,
    app_path: &Path,
    arguments: Vec<std::ffi::OsString>,
) -> std::process::Command {
    use std::os::unix::process::CommandExt as _;

    // Script to wait for the current process to exit and then restart the app.
    // Pass dynamic values as positional parameters to avoid shell interpolation issues.
    let script = r#"
            while kill -0 "$0" 2>/dev/null; do
                sleep 0.1
            done

            app_path="$1"
            shift
            "$app_path" "$@"
            "#;

    #[allow(
        clippy::disallowed_methods,
        reason = "We are restarting ourselves, using std command thus is fine"
    )]
    let mut command = new_std_command("/usr/bin/env");
    command
        .arg("bash")
        .arg("-c")
        .arg(script)
        .arg(app_pid)
        .arg(app_path)
        .args(arguments)
        .process_group(0);
    command
        .env_remove("WAYLAND_SOCKET")
        .env_remove("XDG_ACTIVATION_TOKEN");
    command
}

impl Platform for LinuxPlatform {
    fn platform_info(&self) -> open_gpui::PlatformInfo {
        open_gpui::PlatformInfo {
            os: if cfg!(target_os = "freebsd") {
                open_gpui::OperatingSystem::FreeBsd
            } else {
                open_gpui::OperatingSystem::Linux
            },
            target: open_gpui::PlatformTarget::Native,
        }
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.with_common(|common| common.background_executor.clone())
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.with_common(|common| common.foreground_executor.clone())
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.with_common(|common| common.text_system.clone())
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.connection.borrow().keyboard_layout()
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(open_gpui::DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.with_common(|common| common.callbacks.keyboard_layout_change = Some(callback));
    }

    fn on_thermal_state_change(&self, _callback: NativeAppNotificationHandler) {}

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn power_mode(&self) -> PowerMode {
        if self.connection.borrow().is_headless() {
            return PowerMode::Unknown;
        }
        #[cfg(any(feature = "wayland", feature = "x11"))]
        let windowed = !self.connection.borrow().is_headless();
        self.with_common(|common| {
            #[cfg(any(feature = "wayland", feature = "x11"))]
            if windowed {
                common.start_power_mode_listener();
            }
            common.power_mode
        })
    }

    fn on_power_mode_change(&self, callback: NativeAppNotificationHandler) {
        #[cfg(any(feature = "wayland", feature = "x11"))]
        let windowed = !self.connection.borrow().is_headless();
        self.with_common(|common| {
            if common.power_observation_stopped {
                return;
            }
            common.callbacks.power_mode_change = Some(callback);
            #[cfg(any(feature = "wayland", feature = "x11"))]
            if windowed {
                common.start_power_mode_listener();
            }
        });
    }

    #[cfg(not(any(feature = "wayland", feature = "x11")))]
    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(Err(anyhow!(
            "Idle sleep prevention for {reason:?} requires a Linux windowing backend"
        )))
    }

    #[cfg(any(feature = "wayland", feature = "x11"))]
    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        if self.connection.borrow().is_headless() {
            return Task::ready(Err(anyhow!(
                "Idle sleep prevention for {reason:?} requires a Linux windowing backend"
            )));
        }

        let executor = self.background_executor();
        let (guard_tx, guard_rx) = oneshot::channel();
        executor
            .spawn({
                let executor = executor.clone();
                let reason = reason.to_owned();
                async move {
                    guard_tx
                        .send(inhibit_idle_sleep(reason, executor).await)
                        .ok();
                }
            })
            .detach();
        executor
            .clone()
            .spawn(async move { await_idle_sleep_prevention(guard_rx, &executor).await })
    }

    fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
        let _shutdown = RuntimeShutdown(self);
        if self.stopped.get() {
            return;
        }
        self.connect_initially();
        on_finish_launching();

        let mut event_loop = self
            .event_loop
            .borrow_mut()
            .take()
            .expect("App is already running");
        if !self.stopped.get() {
            event_loop
                .run(None, &mut (), |_| self.apply_pending_mode())
                .log_err();
        }
        self.stop_runtime();

        let quit = self.with_common(|common| {
            common.stop_power_listeners();
            common.callbacks.quit.take()
        });
        if let Some(mut fun) = quit {
            fun();
        }
        #[cfg(feature = "x11")]
        crate::linux::wait_for_clipboard_handovers();
    }

    fn quit(&self) {
        self.stop_runtime();
    }

    fn set_initial_windowing(&self, request: WindowingRequest) {
        if !self.started.get() && !self.stopped.get() {
            *self.initial_windowing.borrow_mut() = Some(request);
        }
    }

    fn request_windowing(&self, request: WindowingRequest) -> Task<anyhow::Result<()>> {
        self.request_mode(request, Box::new(|| Ok(())))
    }

    fn request_windowing_with_validation(
        &self,
        request: WindowingRequest,
        validate: Box<dyn Fn() -> anyhow::Result<()>>,
    ) -> Task<anyhow::Result<()>> {
        self.request_mode(request, validate)
    }

    fn compositor_name(&self) -> &'static str {
        self.connection.borrow().compositor_name()
    }

    fn graphical_environment(&self) -> Option<GraphicalEnvironment> {
        self.connection.borrow().graphical_environment()
    }

    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<std::ffi::OsString>) {
        // get the process id of the current process
        let app_pid = std::process::id().to_string();
        // get the path to the executable
        let app_path = if let Some(path) = binary_path {
            path
        } else {
            match self.app_path() {
                Ok(path) => path,
                Err(err) => {
                    log::error!("Failed to get app path: {:?}", err);
                    return;
                }
            }
        };

        log::info!("Restarting process, using app path: {:?}", app_path);

        let mut command = restart_command(&app_pid, &app_path, arguments);
        if let Some(environment) = self.graphical_environment() {
            environment.apply_to(&mut command);
        }
        let restart_process = command.spawn();

        match restart_process {
            Ok(_) => self.quit(),
            Err(e) => log::error!("failed to spawn restart script: {:?}", e),
        }
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        log::info!("activate is not implemented on Linux, ignoring the call")
    }

    fn hide(&self) {
        log::info!("hide is not implemented on Linux, ignoring the call")
    }

    fn hide_other_apps(&self) {
        log::info!("hide_other_apps is not implemented on Linux, ignoring the call")
    }

    fn unhide_other_apps(&self) {
        log::info!("unhide_other_apps is not implemented on Linux, ignoring the call")
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.connection.borrow().primary_display()
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.connection.borrow().displays()
    }

    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.with_common(|common| common.callbacks.displays_changed = Some(callback));
    }

    #[cfg(feature = "screen-capture")]
    fn is_screen_capture_supported(&self) -> bool {
        self.connection.borrow().is_screen_capture_supported()
    }

    #[cfg(feature = "screen-capture")]
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn open_gpui::ScreenCaptureSource>>>> {
        self.connection.borrow().screen_capture_sources()
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.connection.borrow().active_window()
    }

    fn focused_window(&self) -> PlatformFocusedWindow {
        self.connection.borrow().focused_window()
    }

    fn hovered_window(&self) -> PlatformHoveredWindow {
        self.connection.borrow().hovered_window()
    }

    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.connection.borrow().window_stack()
    }

    fn viewport_capabilities(&self) -> PlatformViewportCapabilities {
        self.connection.borrow().viewport_capabilities()
    }

    fn window_capabilities(
        &self,
        kind: &WindowKind,
        display_id: Option<DisplayId>,
    ) -> PlatformWindowCapabilities {
        self.connection
            .borrow()
            .window_capabilities(kind, display_id)
    }

    fn validate_window_kind(&self, kind: &WindowKind) -> anyhow::Result<()> {
        self.connection.borrow().validate_window_kind(kind)
    }

    fn mouse_button_is_pressed(&self, button: MouseButton) -> Option<bool> {
        self.connection.borrow().mouse_button_is_pressed(button)
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        if self.stopped.get() {
            anyhow::bail!("cannot open a window after platform shutdown");
        }
        self.connection.borrow().open_window(handle, options)
    }

    fn open_url(&self, url: &str) {
        self.connection.borrow().open_uri(url);
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.with_common(|common| common.callbacks.open_urls = Some(callback));
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (done_tx, done_rx) = oneshot::channel();

        #[cfg(not(any(feature = "wayland", feature = "x11")))]
        let _ = (done_tx.send(Ok(None)), options);

        #[cfg(any(feature = "wayland", feature = "x11"))]
        let identifier = self.connection.borrow().window_identifier();

        #[cfg(any(feature = "wayland", feature = "x11"))]
        self.foreground_executor()
            .spawn(async move {
                let title = if options.directories {
                    "Open Folder"
                } else {
                    "Open File"
                };

                let request = match ashpd::desktop::file_chooser::OpenFileRequest::default()
                    .identifier(identifier.await)
                    .modal(true)
                    .title(title)
                    .accept_label(options.prompt.as_ref().map(open_gpui::SharedString::as_str))
                    .multiple(options.multiple)
                    .directory(options.directories)
                    .send()
                    .await
                {
                    Ok(request) => request,
                    Err(err) => {
                        let result = match err {
                            ashpd::Error::PortalNotFound(_) => anyhow!(FILE_PICKER_PORTAL_MISSING),
                            err => err.into(),
                        };
                        let _ = done_tx.send(Err(result));
                        return;
                    }
                };

                let result = match request.response() {
                    Ok(response) => Ok(Some(
                        response
                            .uris()
                            .iter()
                            .filter_map(|uri: &ashpd::Uri| url::Url::parse(uri.as_str()).ok())
                            .filter_map(|uri: url::Url| uri.to_file_path().ok())
                            .collect::<Vec<_>>(),
                    )),
                    Err(ashpd::Error::Response(_)) => Ok(None),
                    Err(e) => Err(e.into()),
                };
                let _ = done_tx.send(result);
            })
            .detach();
        done_rx
    }

    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (done_tx, done_rx) = oneshot::channel();

        #[cfg(not(any(feature = "wayland", feature = "x11")))]
        let _ = (done_tx.send(Ok(None)), directory, suggested_name);

        #[cfg(any(feature = "wayland", feature = "x11"))]
        let identifier = self.connection.borrow().window_identifier();

        #[cfg(any(feature = "wayland", feature = "x11"))]
        self.foreground_executor()
            .spawn({
                let directory = directory.to_owned();
                let suggested_name = suggested_name.map(|s| s.to_owned());

                async move {
                    let mut request_builder =
                        ashpd::desktop::file_chooser::SaveFileRequest::default()
                            .identifier(identifier.await)
                            .modal(true)
                            .title("Save File")
                            .current_folder(directory)
                            .expect("pathbuf should not be nul terminated");

                    if let Some(suggested_name) = suggested_name {
                        request_builder = request_builder.current_name(suggested_name.as_str());
                    }

                    let request = match request_builder.send().await {
                        Ok(request) => request,
                        Err(err) => {
                            let result = match err {
                                ashpd::Error::PortalNotFound(_) => {
                                    anyhow!(FILE_PICKER_PORTAL_MISSING)
                                }
                                err => err.into(),
                            };
                            let _ = done_tx.send(Err(result));
                            return;
                        }
                    };

                    let result = match request.response() {
                        Ok(response) => Ok(response
                            .uris()
                            .first()
                            .and_then(|uri: &ashpd::Uri| url::Url::parse(uri.as_str()).ok())
                            .and_then(|uri: url::Url| uri.to_file_path().ok())),
                        Err(ashpd::Error::Response(_)) => Ok(None),
                        Err(e) => Err(e.into()),
                    };
                    let _ = done_tx.send(result);
                }
            })
            .detach();

        done_rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        // org.freedesktop.portal.FileChooser only supports "pick files" and "pick directories".
        false
    }

    fn reveal_path(&self, path: &Path) {
        self.connection.borrow().reveal_path(path.to_owned());
    }

    fn open_with_system(&self, path: &Path) {
        let path = path.to_owned();
        let environment = self.connection.borrow().graphical_environment();
        self.background_executor()
            .spawn(async move {
                let mut command = new_std_command("xdg-open");
                command.arg(path);
                command
                    .env_remove("WAYLAND_SOCKET")
                    .env_remove("XDG_ACTIVATION_TOKEN");
                if let Some(environment) = environment {
                    environment.apply_to(&mut command);
                }
                #[allow(
                    clippy::disallowed_methods,
                    reason = "running on a background thread, so blocking is fine"
                )]
                command.status().context("invoking xdg-open").log_err();
            })
            .detach();
    }

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.with_common(|common| {
            common.callbacks.quit = Some(callback);
        });
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.with_common(|common| {
            common.callbacks.reopen = Some(callback);
        });
    }

    fn on_system_sleep(&self, callback: NativeAppNotificationHandler) {
        self.with_common(|common| {
            if common.power_observation_stopped {
                return;
            }
            common.callbacks.system_sleep = Some(callback);
            common.start_power_listener();
        });
    }

    fn on_system_wake(&self, callback: NativeAppNotificationHandler) {
        self.with_common(|common| {
            if common.power_observation_stopped {
                return;
            }
            common.callbacks.system_wake = Some(callback);
            common.start_power_listener();
        });
    }

    fn set_app_identity(&self, _identifier: &str, name: &str) {
        self.with_common(|common| common.app_name = Some(name.to_string()));
    }

    fn show_system_notification(&self, notification: open_gpui::SystemNotification) {
        self.with_common(|common| {
            common
                .system_notifications
                .show(common.app_name.as_deref(), notification)
        });
    }

    fn dismiss_system_notification(&self, tag: &str) {
        self.with_common(|common| common.system_notifications.dismiss(tag));
    }

    fn on_system_notification_response(
        &self,
        callback: Box<dyn FnMut(open_gpui::SystemNotificationResponse)>,
    ) {
        self.with_common(|common| {
            let executor = common.foreground_executor.clone();
            common.system_notifications.on_response(&executor, callback)
        });
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.with_common(|common| {
            common.callbacks.app_menu_action = Some(callback);
        });
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.with_common(|common| {
            common.callbacks.will_open_app_menu = Some(callback);
        });
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.with_common(|common| {
            common.callbacks.validate_app_menu_command = Some(callback);
        });
    }

    fn app_path(&self) -> Result<PathBuf> {
        // get the path of the executable of the current process
        let app_path = env::current_exe()?;
        Ok(app_path)
    }

    fn set_menus(&self, menus: Vec<Menu>, _keymap: &Keymap) {
        self.with_common(|common| {
            common.menus = menus.into_iter().map(|menu| menu.owned()).collect();
        })
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.with_common(|common| Some(common.menus.clone()))
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {
        // todo(linux)
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        Err(anyhow::Error::msg(
            "Platform<LinuxPlatform>::path_for_auxiliary_executable is not implemented yet",
        ))
    }

    fn hide_cursor_until_mouse_moves(&self) {
        self.connection.borrow().hide_cursor_until_mouse_moves()
    }

    fn is_cursor_visible(&self) -> bool {
        self.connection.borrow().is_cursor_visible()
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        self.with_common(|common| common.auto_hide_scrollbars)
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        let url = url.to_string();
        let username = username.to_string();
        let password = password.to_vec();
        self.background_executor().spawn(async move {
            let keyring = oo7::Keyring::new().await?;
            keyring.unlock().await?;
            keyring
                .create_item(
                    KEYRING_LABEL,
                    &vec![("url", &url), ("username", &username)],
                    password,
                    true,
                )
                .await?;
            Ok(())
        })
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        let url = url.to_string();
        self.background_executor().spawn(async move {
            let keyring = oo7::Keyring::new().await?;
            keyring.unlock().await?;

            let items = keyring.search_items(&vec![("url", &url)]).await?;

            for item in items.into_iter() {
                if item.label().await.is_ok_and(|label| label == KEYRING_LABEL) {
                    let attributes = item.attributes().await?;
                    let username = attributes
                        .get("username")
                        .context("Cannot find username in stored credentials")?;
                    item.unlock().await?;
                    let secret = item.secret().await?;

                    // we lose the zeroizing capabilities at this boundary,
                    // a current limitation GPUI's credentials api
                    return Ok(Some((username.to_string(), secret.to_vec())));
                } else {
                    continue;
                }
            }
            Ok(None)
        })
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        let url = url.to_string();
        self.background_executor().spawn(async move {
            let keyring = oo7::Keyring::new().await?;
            keyring.unlock().await?;

            let items = keyring.search_items(&vec![("url", &url)]).await?;

            for item in items.into_iter() {
                if item.label().await.is_ok_and(|label| label == KEYRING_LABEL) {
                    item.delete().await?;
                    return Ok(());
                }
            }

            Ok(())
        })
    }

    fn window_appearance(&self) -> WindowAppearance {
        self.with_common(|common| common.appearance)
    }

    fn button_layout(&self) -> Option<WindowButtonLayout> {
        Some(self.with_common(|common| common.button_layout))
    }

    fn register_url_scheme(&self, _: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Err(anyhow!("register_url_scheme unimplemented")))
    }

    fn write_to_primary(&self, item: ClipboardItem) {
        self.connection.borrow().write_to_primary(item)
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        self.connection.borrow().write_to_clipboard(item)
    }

    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.connection.borrow().read_from_primary()
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.connection.borrow().read_from_clipboard()
    }

    fn add_recent_document(&self, _path: &Path) {}
}

/// Opens `target` with the first of the desktop's launchers that succeeds. Returns whether one
/// did.
#[cfg(any(feature = "wayland", feature = "x11"))]
async fn run_open_commands(
    target: &std::ffi::OsStr,
    activation_token: Option<&str>,
    environment: &GraphicalEnvironment,
) -> bool {
    for mut command in open::commands(target) {
        environment.apply_to(&mut command);
        command
            .env_remove("WAYLAND_SOCKET")
            .env_remove("XDG_ACTIVATION_TOKEN");
        if let Some(token) = activation_token {
            command.env("XDG_ACTIVATION_TOKEN", token);
        }
        let program = format!("{:?}", command.get_program());
        match smol::process::Command::from(command).spawn() {
            Ok(mut child) => match child.status().await {
                Ok(status) if status.success() => return true,
                Ok(status) => log::error!("Command {} exited with status: {}", program, status),
                Err(error) => log::error!("Failed to get status from {}: {}", program, error),
            },
            Err(error) => log::error!("Failed to open with {}: {}", program, error),
        }
    }
    false
}

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn open_uri_internal(
    executor: BackgroundExecutor,
    uri: &str,
    activation_token: Option<String>,
    environment: GraphicalEnvironment,
) {
    if let Some(uri) = ashpd::Uri::parse(uri).log_err() {
        executor
            .spawn(async move {
                let opened = run_open_commands(
                    uri.as_str().as_ref(),
                    activation_token.as_deref(),
                    &environment,
                )
                .await;
                if !opened {
                    match ashpd::desktop::open_uri::OpenFileRequest::default()
                        .activation_token(activation_token.map(ashpd::ActivationToken::from))
                        .send_uri(&uri)
                        .await
                        .and_then(|e| e.response())
                    {
                        Ok(()) => {}
                        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {}
                        Err(e) => {
                            log::error!("Failed to open with dbus: {}", e);
                        }
                    }
                }
            })
            .detach();
    }
}

#[cfg(any(feature = "x11", feature = "wayland"))]
pub(super) fn reveal_path_internal(
    executor: BackgroundExecutor,
    path: PathBuf,
    activation_token: Option<String>,
    environment: GraphicalEnvironment,
) {
    executor
        .spawn(async move {
            if let Some(dir) = File::open(path.clone()).log_err() {
                match ashpd::desktop::open_uri::OpenDirectoryRequest::default()
                    .activation_token(activation_token.clone().map(ashpd::ActivationToken::from))
                    .send(&dir.as_fd())
                    .await
                {
                    Ok(_) => return,
                    Err(e) => log::error!("Failed to open with dbus: {}", e),
                }
                let directory = if path.is_dir() {
                    path.as_path()
                } else {
                    path.parent().unwrap_or(Path::new(""))
                };
                run_open_commands(
                    directory.as_os_str(),
                    activation_token.as_deref(),
                    &environment,
                )
                .await;
            }
        })
        .detach();
}

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn is_within_click_distance(a: Point<Pixels>, b: Point<Pixels>) -> bool {
    let diff = a - b;
    diff.x.abs() <= DOUBLE_CLICK_DISTANCE && diff.y.abs() <= DOUBLE_CLICK_DISTANCE
}

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn get_xkb_compose_state(cx: &xkb::Context) -> Option<xkb::compose::State> {
    let mut locales = Vec::default();
    if let Some(locale) = env::var_os("LC_CTYPE") {
        locales.push(locale);
    }
    locales.push(OsString::from("C"));
    let mut state: Option<xkb::compose::State> = None;
    for locale in locales {
        if let Ok(table) =
            xkb::compose::Table::new_from_locale(cx, &locale, xkb::compose::COMPILE_NO_FLAGS)
        {
            state = Some(xkb::compose::State::new(
                &table,
                xkb::compose::STATE_NO_FLAGS,
            ));
            break;
        }
    }
    state
}

#[cfg(feature = "wayland")]
pub(super) const PIPE_READ_TIMEOUT: Duration = Duration::from_secs(4);

#[cfg(feature = "wayland")]
pub(super) fn read_fd_with_timeout(
    mut fd: filedescriptor::FileDescriptor,
    timeout: Duration,
) -> Result<Vec<u8>> {
    fd.set_non_blocking(true)?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let mut poll_fds = [filedescriptor::pollfd {
            fd: fd.as_raw_fd(),
            events: filedescriptor::POLLIN,
            revents: 0,
        }];
        let ready = match filedescriptor::poll(&mut poll_fds, Some(timeout)) {
            Ok(ready) => ready,
            Err(filedescriptor::Error::Poll(err))
                if err.kind() == std::io::ErrorKind::Interrupted =>
            {
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        if ready == 0 {
            anyhow::bail!("timed out waiting for data on pipe after {timeout:?}");
        }
        match fd.read(&mut chunk) {
            Ok(0) => return Ok(buffer),
            Ok(len) => buffer.extend_from_slice(&chunk[..len]),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) const DEFAULT_CURSOR_ICON_NAME: &str = "left_ptr";

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn cursor_style_to_icon_names(style: CursorStyle) -> &'static [&'static str] {
    // Based on cursor names from chromium:
    // https://github.com/chromium/chromium/blob/d3069cf9c973dc3627fa75f64085c6a86c8f41bf/ui/base/cursor/cursor_factory.cc#L113
    match style {
        CursorStyle::Arrow => &[DEFAULT_CURSOR_ICON_NAME],
        CursorStyle::IBeam => &["text", "xterm"],
        CursorStyle::Crosshair => &["crosshair", "cross"],
        CursorStyle::ClosedHand => &["closedhand", "grabbing", "hand2"],
        CursorStyle::OpenHand => &["openhand", "grab", "hand1"],
        CursorStyle::PointingHand => &["pointer", "hand", "hand2"],
        CursorStyle::ResizeLeft => &["w-resize", "left_side"],
        CursorStyle::ResizeRight => &["e-resize", "right_side"],
        CursorStyle::ResizeLeftRight => &["ew-resize", "sb_h_double_arrow"],
        CursorStyle::ResizeUp => &["n-resize", "top_side"],
        CursorStyle::ResizeDown => &["s-resize", "bottom_side"],
        CursorStyle::ResizeUpDown => &["sb_v_double_arrow", "ns-resize"],
        CursorStyle::ResizeUpLeftDownRight => &["size_fdiag", "bd_double_arrow", "nwse-resize"],
        CursorStyle::ResizeUpRightDownLeft => &["size_bdiag", "nesw-resize", "fd_double_arrow"],
        CursorStyle::ResizeColumn => &["col-resize", "sb_h_double_arrow"],
        CursorStyle::ResizeRow => &["row-resize", "sb_v_double_arrow"],
        CursorStyle::IBeamCursorForVerticalLayout => &["vertical-text"],
        CursorStyle::OperationNotAllowed => &["not-allowed", "crossed_circle"],
        CursorStyle::DragLink => &["alias"],
        CursorStyle::DragCopy => &["copy"],
        CursorStyle::ContextualMenu => &["context-menu"],
    }
}

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn log_cursor_icon_warning(message: impl std::fmt::Display) {
    if let Ok(xcursor_path) = env::var("XCURSOR_PATH") {
        log::warn!(
            "{:#}\ncursor icon loading may be failing if XCURSOR_PATH environment variable is invalid. \
                    XCURSOR_PATH overrides the default icon search. Its current value is '{}'",
            message,
            xcursor_path
        );
    } else {
        log::warn!("{:#}", message);
    }
}

#[cfg(any(feature = "wayland", feature = "x11"))]
fn guess_ascii(keycode: Keycode, shift: bool) -> Option<char> {
    let c = match (keycode.raw(), shift) {
        (24, _) => 'q',
        (25, _) => 'w',
        (26, _) => 'e',
        (27, _) => 'r',
        (28, _) => 't',
        (29, _) => 'y',
        (30, _) => 'u',
        (31, _) => 'i',
        (32, _) => 'o',
        (33, _) => 'p',
        (34, false) => '[',
        (34, true) => '{',
        (35, false) => ']',
        (35, true) => '}',
        (38, _) => 'a',
        (39, _) => 's',
        (40, _) => 'd',
        (41, _) => 'f',
        (42, _) => 'g',
        (43, _) => 'h',
        (44, _) => 'j',
        (45, _) => 'k',
        (46, _) => 'l',
        (47, false) => ';',
        (47, true) => ':',
        (48, false) => '\'',
        (48, true) => '"',
        (49, false) => '`',
        (49, true) => '~',
        (51, false) => '\\',
        (51, true) => '|',
        (52, _) => 'z',
        (53, _) => 'x',
        (54, _) => 'c',
        (55, _) => 'v',
        (56, _) => 'b',
        (57, _) => 'n',
        (58, _) => 'm',
        (59, false) => ',',
        (59, true) => '>',
        (60, false) => '.',
        (60, true) => '<',
        (61, false) => '/',
        (61, true) => '?',

        _ => return None,
    };

    Some(c)
}

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn keystroke_from_xkb(
    state: &State,
    mut modifiers: open_gpui::Modifiers,
    keycode: Keycode,
) -> open_gpui::Keystroke {
    let key_utf32 = state.key_get_utf32(keycode);
    let key_utf8 = state.key_get_utf8(keycode);
    let key_sym = state.key_get_one_sym(keycode);

    let key = match key_sym {
        Keysym::Return => "enter".to_owned(),
        Keysym::Prior => "pageup".to_owned(),
        Keysym::Next => "pagedown".to_owned(),
        Keysym::ISO_Left_Tab => "tab".to_owned(),
        Keysym::KP_Prior => "pageup".to_owned(),
        Keysym::KP_Next => "pagedown".to_owned(),
        Keysym::XF86_Back => "back".to_owned(),
        Keysym::XF86_Forward => "forward".to_owned(),
        Keysym::XF86_Cut => "cut".to_owned(),
        Keysym::XF86_Copy => "copy".to_owned(),
        Keysym::XF86_Paste => "paste".to_owned(),
        Keysym::XF86_New => "new".to_owned(),
        Keysym::XF86_Open => "open".to_owned(),
        Keysym::XF86_Save => "save".to_owned(),

        Keysym::comma => ",".to_owned(),
        Keysym::period => ".".to_owned(),
        Keysym::less => "<".to_owned(),
        Keysym::greater => ">".to_owned(),
        Keysym::slash => "/".to_owned(),
        Keysym::question => "?".to_owned(),

        Keysym::semicolon => ";".to_owned(),
        Keysym::colon => ":".to_owned(),
        Keysym::apostrophe => "'".to_owned(),
        Keysym::quotedbl => "\"".to_owned(),

        Keysym::bracketleft => "[".to_owned(),
        Keysym::braceleft => "{".to_owned(),
        Keysym::bracketright => "]".to_owned(),
        Keysym::braceright => "}".to_owned(),
        Keysym::backslash => "\\".to_owned(),
        Keysym::bar => "|".to_owned(),

        Keysym::grave => "`".to_owned(),
        Keysym::asciitilde => "~".to_owned(),
        Keysym::exclam => "!".to_owned(),
        Keysym::at => "@".to_owned(),
        Keysym::numbersign => "#".to_owned(),
        Keysym::dollar => "$".to_owned(),
        Keysym::percent => "%".to_owned(),
        Keysym::asciicircum => "^".to_owned(),
        Keysym::ampersand => "&".to_owned(),
        Keysym::asterisk => "*".to_owned(),
        Keysym::parenleft => "(".to_owned(),
        Keysym::parenright => ")".to_owned(),
        Keysym::minus => "-".to_owned(),
        Keysym::underscore => "_".to_owned(),
        Keysym::equal => "=".to_owned(),
        Keysym::plus => "+".to_owned(),
        Keysym::space => "space".to_owned(),
        Keysym::BackSpace => "backspace".to_owned(),
        Keysym::Tab => "tab".to_owned(),
        Keysym::Delete => "delete".to_owned(),
        Keysym::Escape => "escape".to_owned(),

        Keysym::Left => "left".to_owned(),
        Keysym::Right => "right".to_owned(),
        Keysym::Up => "up".to_owned(),
        Keysym::Down => "down".to_owned(),
        Keysym::Home => "home".to_owned(),
        Keysym::End => "end".to_owned(),
        Keysym::Insert => "insert".to_owned(),

        _ => {
            let name = xkb::keysym_get_name(key_sym).to_lowercase();
            if key_sym.is_keypad_key() {
                name.replace("kp_", "")
            } else if let Some(key) = key_utf8.chars().next()
                && key_utf8.len() == 1
                && key.is_ascii()
            {
                if key.is_ascii_graphic() {
                    key_utf8.to_lowercase()
                // map ctrl-a to `a`
                // ctrl-0..9 may emit control codes like ctrl-[, but
                // we don't want to map them to `[`
                } else if key_utf32 <= 0x1f
                    && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
                {
                    ((key_utf32 as u8 + 0x40) as char)
                        .to_ascii_lowercase()
                        .to_string()
                } else {
                    name
                }
            } else if let Some(key_en) = guess_ascii(keycode, modifiers.shift) {
                String::from(key_en)
            } else {
                name
            }
        }
    };

    if modifiers.shift {
        // we only include the shift for upper-case letters by convention,
        // so don't include for numbers and symbols, but do include for
        // tab/enter, etc.
        if key.chars().count() == 1 && key.to_lowercase() == key.to_uppercase() {
            modifiers.shift = false;
        }
    }

    // Ignore control characters (and DEL) for the purposes of key_char
    let key_char =
        (key_utf32 >= 32 && key_utf32 != 127 && !key_utf8.is_empty()).then_some(key_utf8);

    open_gpui::Keystroke {
        modifiers,
        key,
        key_char,
    }
}

/**
 * Returns which symbol the dead key represents
 * <https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_key_values#dead_keycodes_for_linux>
 */
#[cfg(any(feature = "wayland", feature = "x11"))]
pub fn keystroke_underlying_dead_key(keysym: Keysym) -> Option<String> {
    match keysym {
        Keysym::dead_grave => Some("`".to_owned()),
        Keysym::dead_acute => Some("´".to_owned()),
        Keysym::dead_circumflex => Some("^".to_owned()),
        Keysym::dead_tilde => Some("~".to_owned()),
        Keysym::dead_macron => Some("¯".to_owned()),
        Keysym::dead_breve => Some("˘".to_owned()),
        Keysym::dead_abovedot => Some("˙".to_owned()),
        Keysym::dead_diaeresis => Some("¨".to_owned()),
        Keysym::dead_abovering => Some("˚".to_owned()),
        Keysym::dead_doubleacute => Some("˝".to_owned()),
        Keysym::dead_caron => Some("ˇ".to_owned()),
        Keysym::dead_cedilla => Some("¸".to_owned()),
        Keysym::dead_ogonek => Some("˛".to_owned()),
        Keysym::dead_iota => Some("ͅ".to_owned()),
        Keysym::dead_voiced_sound => Some("゙".to_owned()),
        Keysym::dead_semivoiced_sound => Some("゚".to_owned()),
        Keysym::dead_belowdot => Some("̣̣".to_owned()),
        Keysym::dead_hook => Some("̡".to_owned()),
        Keysym::dead_horn => Some("̛".to_owned()),
        Keysym::dead_stroke => Some("̶̶".to_owned()),
        Keysym::dead_abovecomma => Some("̓̓".to_owned()),
        Keysym::dead_abovereversedcomma => Some("ʽ".to_owned()),
        Keysym::dead_doublegrave => Some("̏".to_owned()),
        Keysym::dead_belowring => Some("˳".to_owned()),
        Keysym::dead_belowmacron => Some("̱".to_owned()),
        Keysym::dead_belowcircumflex => Some("ꞈ".to_owned()),
        Keysym::dead_belowtilde => Some("̰".to_owned()),
        Keysym::dead_belowbreve => Some("̮".to_owned()),
        Keysym::dead_belowdiaeresis => Some("̤".to_owned()),
        Keysym::dead_invertedbreve => Some("̯".to_owned()),
        Keysym::dead_belowcomma => Some("̦".to_owned()),
        Keysym::dead_currency => None,
        Keysym::dead_lowline => None,
        Keysym::dead_aboveverticalline => None,
        Keysym::dead_belowverticalline => None,
        Keysym::dead_longsolidusoverlay => None,
        Keysym::dead_a => None,
        Keysym::dead_A => None,
        Keysym::dead_e => None,
        Keysym::dead_E => None,
        Keysym::dead_i => None,
        Keysym::dead_I => None,
        Keysym::dead_o => None,
        Keysym::dead_O => None,
        Keysym::dead_u => None,
        Keysym::dead_U => None,
        Keysym::dead_small_schwa => Some("ə".to_owned()),
        Keysym::dead_capital_schwa => Some("Ə".to_owned()),
        Keysym::dead_greek => None,
        _ => None,
    }
}
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn modifiers_from_xkb(keymap_state: &State) -> open_gpui::Modifiers {
    let shift = keymap_state.mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE);
    let alt = keymap_state.mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE);
    let control = keymap_state.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE);
    let platform = keymap_state.mod_name_is_active(xkb::MOD_NAME_LOGO, xkb::STATE_MODS_EFFECTIVE);
    open_gpui::Modifiers {
        shift,
        alt,
        control,
        platform,
        function: false,
    }
}

#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn capslock_from_xkb(keymap_state: &State) -> open_gpui::Capslock {
    let on = keymap_state.mod_name_is_active(xkb::MOD_NAME_CAPS, xkb::STATE_MODS_EFFECTIVE);
    open_gpui::Capslock { on }
}

/// Resolve a Linux `dev_t` to PCI vendor/device IDs via sysfs, returning a
/// [`CompositorGpuHint`] that the GPU adapter selection code can use to
/// prioritize the compositor's rendering device.
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(super) fn compositor_gpu_hint_from_dev_t(
    dev: u64,
) -> Option<open_gpui_wgpu::CompositorGpuHint> {
    fn dev_major(dev: u64) -> u32 {
        ((dev >> 8) & 0xfff) as u32 | (((dev >> 32) & !0xfff) as u32)
    }

    fn dev_minor(dev: u64) -> u32 {
        (dev & 0xff) as u32 | (((dev >> 12) & !0xff) as u32)
    }

    fn read_sysfs_hex_id(path: &str) -> Option<u32> {
        let content = std::fs::read_to_string(path).ok()?;
        let trimmed = content.trim().strip_prefix("0x").unwrap_or(content.trim());
        u32::from_str_radix(trimmed, 16).ok()
    }

    let major = dev_major(dev);
    let minor = dev_minor(dev);

    let vendor_path = format!("/sys/dev/char/{major}:{minor}/device/vendor");
    let device_path = format!("/sys/dev/char/{major}:{minor}/device/device");

    let vendor_id = read_sysfs_hex_id(&vendor_path)?;
    let device_id = read_sysfs_hex_id(&device_path)?;

    log::info!(
        "Compositor GPU hint: vendor={:#06x}, device={:#06x} (from dev {major}:{minor})",
        vendor_id,
        device_id,
    );

    Some(open_gpui_wgpu::CompositorGpuHint {
        vendor_id,
        device_id,
    })
}

#[cfg(any(feature = "wayland", feature = "x11"))]
async fn inhibit_idle_sleep(reason: String, executor: BackgroundExecutor) -> Result<ActivityGuard> {
    let proxy = InhibitProxy::new()
        .await
        .context("Idle sleep prevention portal is unavailable")?;
    let request = proxy
        .inhibit(
            None,
            BitFlags::from(InhibitFlags::Suspend),
            InhibitOptions::default().set_reason(reason.as_str()),
        )
        .await
        .context("Failed to request idle sleep prevention")?;
    request
        .response()
        .context("Idle sleep prevention request was rejected")?;
    Ok(release_on_drop(request, executor))
}

#[cfg(any(feature = "wayland", feature = "x11"))]
fn release_on_drop(request: Request<()>, executor: BackgroundExecutor) -> ActivityGuard {
    ActivityGuard::new(move || {
        executor
            .spawn(async move {
                request
                    .close()
                    .await
                    .context("Failed to release idle sleep prevention")
                    .log_err();
            })
            .detach();
    })
}

#[cfg(any(feature = "wayland", feature = "x11"))]
async fn await_idle_sleep_prevention(
    guard_rx: oneshot::Receiver<Result<ActivityGuard>>,
    executor: &BackgroundExecutor,
) -> Result<ActivityGuard> {
    match futures::future::select(guard_rx, executor.timer(Duration::from_secs(10))).await {
        futures::future::Either::Left((Ok(result), _)) => result,
        futures::future::Either::Left((Err(_), _)) => {
            Err(anyhow!("Idle sleep prevention request was abandoned"))
        }
        futures::future::Either::Right(_) => Err(anyhow!(
            "Idle sleep prevention acquisition timed out after 10 seconds"
        )),
    }
}

#[cfg(test)]
#[path = "idle_sleep_prevention_tests.rs"]
mod idle_sleep_prevention_tests;

#[cfg(all(test, target_os = "linux", any(feature = "wayland", feature = "x11")))]
#[path = "idle_sleep_portal_tests.rs"]
mod idle_sleep_portal_tests;

#[cfg(test)]
#[path = "restart_arguments_tests.rs"]
mod restart_arguments_tests;

#[cfg(test)]
#[path = "windowing_tests.rs"]
mod windowing_tests;

#[cfg(all(test, feature = "x11", feature = "wayland"))]
#[path = "native_windowing_tests.rs"]
mod native_windowing_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "wayland", feature = "x11"))]
    use open_gpui::{Point, px};

    #[test]
    fn power_mode_reports_update_before_notification_and_without_an_observer() {
        let event_loop = calloop::EventLoop::<()>::try_new().unwrap();
        let (common, _, _) = LinuxCommon::new(event_loop.get_signal());
        let common = std::cell::RefCell::new(common);
        assert_eq!(common.borrow().power_mode, PowerMode::Unknown);
        assert!(
            common
                .borrow_mut()
                .handle_system_power_event(SystemPowerEvent::ModeChanged(PowerMode::LowPower))
                .is_none()
        );
        assert_eq!(common.borrow().power_mode, PowerMode::LowPower);

        let (sender, receiver) = std::sync::mpsc::channel();
        common.borrow_mut().callbacks.power_mode_change = Some(Arc::new(move || {
            sender.send(()).unwrap();
        }));
        for mode in [
            PowerMode::LowPower,
            PowerMode::Balanced,
            PowerMode::Balanced,
            PowerMode::HighPower,
            PowerMode::Unknown,
            PowerMode::Unknown,
        ] {
            let callback = common
                .borrow_mut()
                .handle_system_power_event(SystemPowerEvent::ModeChanged(mode));
            assert_eq!(common.borrow().power_mode, mode);
            if let Some(callback) = callback {
                callback();
            }
        }
        assert_eq!(receiver.try_iter().count(), 3);
        common.borrow_mut().stop_power_listeners();
        assert!(
            common
                .borrow_mut()
                .handle_system_power_event(SystemPowerEvent::ModeChanged(PowerMode::HighPower))
                .is_none()
        );
        assert_eq!(common.borrow().power_mode, PowerMode::Unknown);
    }

    #[test]
    fn headless_power_queries_stay_unknown_and_shutdown_releases_and_refuses_callbacks() {
        let platform = LinuxPlatform::new(WindowingModes::HEADLESS);
        assert_eq!(platform.power_mode(), PowerMode::Unknown);
        assert_eq!(platform.thermal_state(), ThermalState::Nominal);

        let retained = Arc::new(());
        let weak = Arc::downgrade(&retained);
        platform.on_power_mode_change(Arc::new(move || {
            let _ = &retained;
        }));
        assert!(weak.upgrade().is_some());
        assert_eq!(platform.power_mode(), PowerMode::Unknown);
        #[cfg(any(feature = "wayland", feature = "x11"))]
        platform.with_common(|common| {
            assert!(common.power_mode_listener.is_none());
        });

        platform.quit();
        assert!(weak.upgrade().is_none());
        assert_eq!(platform.power_mode(), PowerMode::Unknown);

        let retained = Arc::new(());
        let weak = Arc::downgrade(&retained);
        let callback: NativeAppNotificationHandler = Arc::new(move || {
            let _ = &retained;
        });
        platform.on_power_mode_change(callback.clone());
        platform.on_system_sleep(callback.clone());
        platform.on_system_wake(callback);
        assert!(weak.upgrade().is_none());
        platform.with_common(|common| {
            for event in [
                SystemPowerEvent::Sleep,
                SystemPowerEvent::Wake,
                SystemPowerEvent::ModeChanged(PowerMode::LowPower),
            ] {
                assert!(common.handle_system_power_event(event).is_none());
            }
        });
        assert_eq!(platform.power_mode(), PowerMode::Unknown);
    }

    #[test]
    fn system_power_callbacks_keep_event_order_and_cross_thread_custody() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut callbacks = PlatformHandlers::default();
        assert!(
            callbacks
                .system_power_callback(SystemPowerEvent::Sleep)
                .is_none()
        );
        assert!(
            callbacks
                .system_power_callback(SystemPowerEvent::Wake)
                .is_none()
        );

        callbacks.system_sleep = Some(Arc::new({
            let sender = sender.clone();
            move || sender.send("sleep").unwrap()
        }));
        assert!(
            callbacks
                .system_power_callback(SystemPowerEvent::Wake)
                .is_none()
        );
        callbacks.system_wake = Some(Arc::new({
            let sender = sender.clone();
            move || sender.send("wake").unwrap()
        }));

        let sleep = callbacks
            .system_power_callback(SystemPowerEvent::Sleep)
            .unwrap();
        let wake = callbacks
            .system_power_callback(SystemPowerEvent::Wake)
            .unwrap();
        callbacks.system_sleep = Some(Arc::new({
            let sender = sender.clone();
            move || sender.send("replacement sleep").unwrap()
        }));
        let replacement = callbacks
            .system_power_callback(SystemPowerEvent::Sleep)
            .unwrap();
        drop(callbacks);

        std::thread::spawn(move || {
            sleep();
            wake();
            replacement();
        })
        .join()
        .unwrap();
        drop(sender);

        assert_eq!(
            receiver.into_iter().collect::<Vec<_>>(),
            ["sleep", "wake", "replacement sleep"]
        );
    }

    #[cfg(any(feature = "wayland", feature = "x11"))]
    #[test]
    fn test_is_within_click_distance() {
        let zero = Point::new(px(0.0), px(0.0));
        assert!(is_within_click_distance(zero, Point::new(px(5.0), px(5.0))));
        assert!(is_within_click_distance(
            zero,
            Point::new(px(-4.9), px(5.0))
        ));
        assert!(is_within_click_distance(
            Point::new(px(3.0), px(2.0)),
            Point::new(px(-2.0), px(-2.0))
        ));
        assert!(!is_within_click_distance(
            zero,
            Point::new(px(5.0), px(5.1))
        ),);
    }

    #[cfg(feature = "wayland")]
    mod read_fd_with_timeout {
        use std::io::Write as _;
        use std::time::{Duration, Instant};

        use super::super::{PIPE_READ_TIMEOUT, read_fd_with_timeout};

        #[test]
        fn reads_data_written_before_close() {
            let mut pipe = filedescriptor::Pipe::new().unwrap();
            pipe.write.write_all(b"hello clipboard").unwrap();
            drop(pipe.write);

            let bytes = read_fd_with_timeout(pipe.read, PIPE_READ_TIMEOUT).unwrap();
            assert_eq!(bytes, b"hello clipboard");
        }

        #[test]
        fn returns_empty_when_writer_closes_without_writing() {
            let pipe = filedescriptor::Pipe::new().unwrap();
            drop(pipe.write);

            let bytes = read_fd_with_timeout(pipe.read, PIPE_READ_TIMEOUT).unwrap();
            assert!(bytes.is_empty());
        }

        #[test]
        fn times_out_when_writer_never_writes() {
            let pipe = filedescriptor::Pipe::new().unwrap();
            let _open_writer = pipe.write;

            let timeout = Duration::from_millis(50);
            let started = Instant::now();
            let result = read_fd_with_timeout(pipe.read, timeout);
            let elapsed = started.elapsed();

            let err = result.unwrap_err();
            assert!(
                err.to_string().contains("timed out"),
                "unexpected error: {err}"
            );
            assert!(elapsed >= timeout, "returned before the timeout elapsed");
        }

        #[test]
        fn times_out_when_writer_stalls_after_partial_write() {
            let mut pipe = filedescriptor::Pipe::new().unwrap();
            pipe.write.write_all(b"partial").unwrap();
            let _open_writer = pipe.write;

            let err = read_fd_with_timeout(pipe.read, Duration::from_millis(50)).unwrap_err();
            assert!(
                err.to_string().contains("timed out"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn slow_writer_resets_deadline_between_chunks() {
            let pipe = filedescriptor::Pipe::new().unwrap();
            let chunks = 12;
            let gap = Duration::from_millis(40);
            let timeout = Duration::from_millis(400);

            let writer = std::thread::spawn({
                let mut write = pipe.write;
                move || {
                    for _ in 0..chunks {
                        std::thread::sleep(gap);
                        write.write_all(&[b'x'; 1000]).unwrap();
                    }
                }
            });
            let bytes = read_fd_with_timeout(pipe.read, timeout).unwrap();
            writer.join().unwrap();
            assert_eq!(bytes, vec![b'x'; 1000 * chunks]);
        }

        #[test]
        fn reads_payload_larger_than_pipe_capacity() {
            let pipe = filedescriptor::Pipe::new().unwrap();
            let payload = vec![b'z'; 1024 * 1024];

            let writer = std::thread::spawn({
                let mut write = pipe.write;
                let payload = payload.clone();
                move || write.write_all(&payload).unwrap()
            });
            let bytes = read_fd_with_timeout(pipe.read, PIPE_READ_TIMEOUT).unwrap();
            writer.join().unwrap();
            assert_eq!(bytes, payload);
        }
    }
}

#[cfg(test)]
mod display_mode_tests {
    use std::{cell::Cell, process::Command};

    #[cfg(feature = "wayland")]
    use open_gpui::{AppContext as _, Context, IntoElement, Render, Window, WindowOptions};
    use open_gpui::{Application, AsyncApp, QuitMode};

    use super::*;

    /// Set in the child process that runs a scenario, naming the scenario to run.
    const SCENARIO_ENV: &str = "GPUI_LINUX_DISPLAY_MODE_SCENARIO";
    const MISSING_WAYLAND_DISPLAY: &str = "/gpui-test/wayland-display-does-not-exist";

    #[cfg(feature = "wayland")]
    #[test]
    fn failed_wayland_attach_preserves_headless_app_and_allows_retry() {
        run_scenario(
            "failed_wayland_attach_preserves_headless_app_and_allows_retry",
            WindowingModes::all(),
            |cx| {
                Box::pin(async move {
                    let entity = cx.new(|_| 1usize);
                    let (release_task, wait_for_release) = oneshot::channel();
                    let queued_task_ran = Rc::new(Cell::new(false));
                    let queued_task = cx.spawn({
                        let queued_task_ran = queued_task_ran.clone();
                        async move |_| {
                            wait_for_release
                                .await
                                .expect("queued task release sender remains alive");
                            queued_task_ran.set(true);
                        }
                    });

                    let first_attach = cx.update(|cx| {
                        cx.request_windowing(WindowingRequest::Windowed(
                            GraphicalEnvironment::detect(),
                        ))
                    });
                    assert_error_contains(
                        first_attach.await,
                        "failed to connect to Wayland compositor",
                    );
                    entity.update(cx, |value, _| *value += 1);
                    release_task
                        .send(())
                        .expect("queued task remains attached to the executor");
                    queued_task.await;
                    assert!(queued_task_ran.get());

                    let second_attach = cx.update(|cx| {
                        cx.request_windowing(WindowingRequest::Windowed(
                            GraphicalEnvironment::detect(),
                        ))
                    });
                    assert_error_contains(
                        second_attach.await,
                        "failed to connect to Wayland compositor",
                    );
                    entity.update(cx, |value, _| *value += 1);
                    assert_eq!(entity.read_with(cx, |value, _| *value), 3);
                })
            },
        );
    }

    #[cfg(feature = "wayland")]
    #[test]
    fn headless_windows_block_switching_to_a_display_server() {
        run_scenario(
            "headless_windows_block_switching_to_a_display_server",
            WindowingModes::all(),
            |cx| {
                Box::pin(async move {
                    let window = cx
                        .update(|cx| {
                            cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Blank))
                        })
                        .expect("headless mode opens headless windows");
                    let attach = cx.update(|cx| {
                        cx.request_windowing(WindowingRequest::Windowed(
                            GraphicalEnvironment::detect(),
                        ))
                    });
                    assert_error_contains(attach.await, "while windows are open");

                    cx.update(|cx| window.update(cx, |_, window, cx| window.remove_window(cx)))
                        .expect("close the headless window");
                    let attach = cx.update(|cx| {
                        cx.request_windowing(WindowingRequest::Windowed(
                            GraphicalEnvironment::detect(),
                        ))
                    });
                    assert_error_contains(attach.await, "failed to connect to Wayland compositor");
                })
            },
        );
    }

    #[test]
    fn display_modes_limit_switching() {
        run_scenario(
            "display_modes_limit_switching",
            WindowingModes::HEADLESS,
            |cx| {
                Box::pin(async move {
                    let attach = cx.update(|cx| {
                        cx.request_windowing(WindowingRequest::Windowed(
                            GraphicalEnvironment::detect(),
                        ))
                    });
                    assert_error_contains(attach.await, "names no display server that");
                    assert_eq!(cx.update(|cx| cx.compositor_name()), "headless");
                    assert!(cx.update(|cx| cx.graphical_environment()).is_none());

                    let detach = cx.update(|cx| cx.request_windowing(WindowingRequest::Headless));
                    assert_error_contains(detach.await, "already headless");
                })
            },
        );
    }

    #[cfg(feature = "wayland")]
    struct Blank;

    #[cfg(feature = "wayland")]
    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            open_gpui::Empty
        }
    }

    type Scenario = fn(&mut AsyncApp) -> futures::future::LocalBoxFuture<'_, ()>;

    /// Runs `scenario` on a headless platform in a child process, where `WAYLAND_DISPLAY` names a
    /// socket that doesn't exist.
    fn run_scenario(name: &str, modes: WindowingModes, scenario: Scenario) {
        if std::env::var(SCENARIO_ENV).as_deref() == Ok(name) {
            let platform = Rc::new(LinuxPlatform::new(modes));
            platform.set_initial_windowing(WindowingRequest::Headless);
            let completed = Rc::new(Cell::new(false));
            Application::with_platform(platform)
                .with_quit_mode(QuitMode::Explicit)
                .run({
                    let completed = completed.clone();
                    move |cx| {
                        cx.spawn(async move |cx| {
                            scenario(cx).await;
                            completed.set(true);
                            cx.update(|cx| cx.quit());
                        })
                        .detach();
                    }
                });
            assert!(completed.get(), "scenario did not complete");
            return;
        }

        // Test names don't include the crate's name.
        let module = module_path!()
            .split_once("::")
            .map_or(module_path!(), |(_, module)| module);
        let test_name = format!("{module}::{name}");
        #[allow(
            clippy::disallowed_methods,
            reason = "the test thread has nothing else to do while the child runs"
        )]
        let output = Command::new(std::env::current_exe().expect("current test executable"))
            .args(["--exact", &test_name, "--nocapture"])
            .env(SCENARIO_ENV, name)
            .env("WAYLAND_DISPLAY", MISSING_WAYLAND_DISPLAY)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("XDG_ACTIVATION_TOKEN")
            .output()
            .expect("run the scenario in a child process");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "scenario failed\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_error_contains(result: anyhow::Result<()>, expected: &str) {
        let error = result.expect_err("the transition fails");
        assert!(
            format!("{error:#}").contains(expected),
            "expected an error containing {expected:?}, got: {error:#}"
        );
    }
}
