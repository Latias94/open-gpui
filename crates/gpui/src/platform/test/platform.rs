use super::display::TestDisplayChangeCallback;
#[cfg(any(test, feature = "test-support"))]
use crate::ClipboardReadError;
use crate::{
    ActivityGuard, AnyWindowHandle, BackgroundExecutor, Bounds, ClipboardItem, CursorStyle,
    DevicePixels, DisplayId, DummyKeyboardMapper, ForegroundExecutor, Keymap, MouseButton,
    NativeAppNotificationHandler, NoopTextSystem, OwnedMenu, PathPromptOptions, Pixels, Platform,
    PlatformDisplay, PlatformDisplaySnapshot, PlatformFocusedWindow, PlatformHeadlessRenderer,
    PlatformHoveredWindow, PlatformKeyboardLayout, PlatformKeyboardMapper,
    PlatformNativeDragHysteresis, PlatformTextSystem, PlatformWindowActivationSupport,
    PlatformWindowCapabilities, PlatformWindowCommandOutcome, PlatformWindowCreationCapabilities,
    PlatformWindowHitStack, PlatformWindowMutationCapabilities, Point, PowerMode, PromptButton,
    ScreenCaptureFrame, ScreenCaptureSource, ScreenCaptureStream, SharedString, SourceMetadata,
    SystemNotification, SystemNotificationResponse, Task, TestDisplay, TestWindow, ThermalState,
    WindowAppearance, WindowCoordinateSpace, WindowCreationSupport, WindowInitialPresentationOrder,
    WindowMutationSupport, WindowParams, size,
};
use anyhow::Result;
use futures::channel::oneshot;
use open_gpui_collections::VecDeque;
use parking_lot::Mutex;
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[cfg(test)]
pub(crate) type TestWindowingRequestHandler =
    Rc<dyn Fn(crate::WindowingRequest, Box<dyn Fn() -> Result<()>>) -> Task<Result<()>>>;

/// TestPlatform implements the Platform trait for use in tests.
pub(crate) struct TestPlatform {
    pub(crate) platform_info: Cell<crate::PlatformInfo>,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,

    pub(crate) active_window: RefCell<Option<TestWindow>>,
    pub(crate) focused_window_available: RefCell<bool>,
    pub(crate) hovered_window_available: RefCell<bool>,
    pub(crate) hovered_window: RefCell<Option<TestWindow>>,
    window_stack: RefCell<Option<Vec<TestWindow>>>,
    window_hit_stack: RefCell<Option<PlatformWindowHitStack>>,
    native_drag_hysteresis: RefCell<Option<PlatformNativeDragHysteresis>>,
    platform_viewport_windows: RefCell<bool>,
    pointer_input_mutation_supported: RefCell<bool>,
    window_creation_capabilities_override: RefCell<Option<PlatformWindowCreationCapabilities>>,
    window_mutation_capabilities_override: RefCell<Option<PlatformWindowMutationCapabilities>>,
    next_window_map_error: RefCell<Option<String>>,
    last_created_window: RefCell<Option<TestWindow>>,
    next_window_close_during_map: RefCell<bool>,
    next_window_creation_show_fact: RefCell<Option<bool>>,
    next_window_initial_presentation_command_outcomes:
        RefCell<Option<VecDeque<PlatformWindowCommandOutcome>>>,
    next_window_close_during_initial_presentation: RefCell<bool>,
    next_window_defer_frame_requests: RefCell<bool>,
    displays: RefCell<Vec<TestDisplay>>,
    displays_changed_callback: TestDisplayChangeCallback,
    display_generation: Cell<u64>,
    display_snapshot_query_count: Cell<usize>,
    active_cursor: Mutex<CursorStyle>,
    pressed_mouse_buttons: Mutex<Option<Vec<MouseButton>>>,
    current_clipboard_item: Mutex<Option<ClipboardItem>>,
    #[cfg(any(test, feature = "test-support"))]
    next_clipboard_read: RefCell<Option<Task<Result<Option<ClipboardItem>, ClipboardReadError>>>>,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    current_primary_item: Mutex<Option<ClipboardItem>>,
    #[cfg(target_os = "macos")]
    current_find_pasteboard_item: Mutex<Option<ClipboardItem>>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) prompts: RefCell<TestPrompts>,
    screen_capture_sources: RefCell<Vec<TestScreenCaptureSource>>,
    pub opened_url: RefCell<Option<String>>,
    pub text_system: Arc<dyn PlatformTextSystem>,
    pub expect_restart: RefCell<Option<oneshot::Sender<(Option<PathBuf>, Vec<OsString>)>>>,
    #[cfg(test)]
    pub(crate) window_appearance_requests: RefCell<Vec<Option<WindowAppearance>>>,
    #[cfg(test)]
    pub(crate) initial_windowing_requests: RefCell<Vec<crate::WindowingRequest>>,
    #[cfg(test)]
    pub(crate) windowing_request_handler: RefCell<Option<TestWindowingRequestHandler>>,
    idle_sleep_prevention_count: Arc<AtomicUsize>,
    idle_sleep_prevention_delay: Cell<Duration>,
    idle_sleep_prevention_fails: Cell<bool>,
    quit_requested: RefCell<bool>,
    quit_callback: RefCell<Option<Box<dyn FnMut() -> bool>>>,
    open_urls_callback: RefCell<Option<Box<dyn FnMut(Vec<String>)>>>,
    reopen_callback: RefCell<Option<Box<dyn FnMut()>>>,
    system_notifications: RefCell<TestSystemNotifications>,
    system_wake_callback: RefCell<Option<NativeAppNotificationHandler>>,
    system_sleep_callback: RefCell<Option<NativeAppNotificationHandler>>,
    app_lifecycle_callback: RefCell<Option<Box<dyn FnMut(crate::AppLifecyclePhase)>>>,
    memory_warning_callback: RefCell<Option<Box<dyn FnMut()>>>,
    thermal_state: Cell<ThermalState>,
    thermal_state_callback: RefCell<Option<NativeAppNotificationHandler>>,
    power_mode: Cell<PowerMode>,
    power_mode_callback: RefCell<Option<NativeAppNotificationHandler>>,
    defer_next_launch: Cell<bool>,
    will_open_app_menu_callback: RefCell<Option<Box<dyn FnMut()>>>,
    app_menu_action_callback: RefCell<Option<Box<dyn FnMut(&dyn crate::Action)>>>,
    validate_app_menu_command_callback: RefCell<Option<Box<dyn FnMut(&dyn crate::Action) -> bool>>>,
    menus: RefCell<Vec<OwnedMenu>>,
    headless_renderer_factory:
        Option<Box<dyn Fn() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>>>>,
    weak: Weak<Self>,
}

#[derive(Clone)]
/// A fake screen capture source, used for testing.
pub struct TestScreenCaptureSource {}

/// A fake screen capture stream, used for testing.
pub struct TestScreenCaptureStream {}

impl ScreenCaptureSource for TestScreenCaptureSource {
    fn metadata(&self) -> Result<SourceMetadata> {
        Ok(SourceMetadata {
            id: 0,
            is_main: None,
            label: None,
            resolution: size(DevicePixels(1), DevicePixels(1)),
        })
    }

    fn stream(
        &self,
        _foreground_executor: &ForegroundExecutor,
        _frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>> {
        let (mut tx, rx) = oneshot::channel();
        let stream = TestScreenCaptureStream {};
        tx.send(Ok(Box::new(stream) as Box<dyn ScreenCaptureStream>))
            .ok();
        rx
    }
}

impl ScreenCaptureStream for TestScreenCaptureStream {
    fn metadata(&self) -> Result<SourceMetadata> {
        TestScreenCaptureSource {}.metadata()
    }
}

#[cfg(any(test, feature = "test-support"))]
struct TestPrompt {
    msg: String,
    detail: Option<String>,
    answers: Vec<String>,
    tx: oneshot::Sender<usize>,
}

#[derive(Default)]
pub(crate) struct TestSystemNotifications {
    app_identity: Option<(SharedString, SharedString)>,
    shown: Vec<SystemNotification>,
    delivered: Vec<SystemNotification>,
    dismissed: Vec<SharedString>,
    response_callback: Option<Box<dyn FnMut(SystemNotificationResponse)>>,
}

#[derive(Default)]
#[cfg(any(test, feature = "test-support"))]
pub(crate) struct TestPrompts {
    multiple_choice: VecDeque<TestPrompt>,
    new_path: VecDeque<(PathBuf, oneshot::Sender<Result<Option<PathBuf>>>)>,
    paths: VecDeque<(
        PathPromptOptions,
        oneshot::Sender<Result<Option<Vec<PathBuf>>>>,
    )>,
}

impl TestPlatform {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_next_clipboard_read(
        &self,
        read: Task<Result<Option<ClipboardItem>, ClipboardReadError>>,
    ) {
        *self.next_clipboard_read.borrow_mut() = Some(read);
    }

    #[cfg(test)]
    pub(crate) fn defer_next_launch(&self) {
        self.defer_next_launch.set(true);
    }

    pub fn new(executor: BackgroundExecutor, foreground_executor: ForegroundExecutor) -> Rc<Self> {
        Self::with_platform(
            executor,
            foreground_executor,
            Arc::new(NoopTextSystem),
            None,
        )
    }

    pub fn with_text_system(
        executor: BackgroundExecutor,
        foreground_executor: ForegroundExecutor,
        text_system: Arc<dyn PlatformTextSystem>,
    ) -> Rc<Self> {
        Self::with_platform(executor, foreground_executor, text_system, None)
    }

    pub fn with_platform(
        executor: BackgroundExecutor,
        foreground_executor: ForegroundExecutor,
        text_system: Arc<dyn PlatformTextSystem>,
        headless_renderer_factory: Option<
            Box<dyn Fn() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>>>,
        >,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak| TestPlatform {
            platform_info: Cell::new(crate::PlatformInfo {
                os: {
                    #[cfg(target_os = "macos")]
                    {
                        crate::OperatingSystem::Mac
                    }
                    #[cfg(target_os = "windows")]
                    {
                        crate::OperatingSystem::Windows
                    }
                    #[cfg(target_os = "linux")]
                    {
                        crate::OperatingSystem::Linux
                    }
                    #[cfg(target_os = "freebsd")]
                    {
                        crate::OperatingSystem::FreeBsd
                    }
                    #[cfg(target_os = "ios")]
                    {
                        crate::OperatingSystem::Ios
                    }
                    #[cfg(target_os = "android")]
                    {
                        crate::OperatingSystem::Android
                    }
                    #[cfg(target_family = "wasm")]
                    {
                        crate::OperatingSystem::Unknown
                    }
                    #[cfg(not(any(
                        target_os = "macos",
                        target_os = "windows",
                        target_os = "linux",
                        target_os = "freebsd",
                        target_os = "ios",
                        target_os = "android",
                        target_family = "wasm"
                    )))]
                    compile_error!("unsupported test platform operating system")
                },
                target: if cfg!(target_family = "wasm") {
                    crate::PlatformTarget::Web
                } else {
                    crate::PlatformTarget::Native
                },
            }),
            background_executor: executor,
            foreground_executor,
            #[cfg(any(test, feature = "test-support"))]
            prompts: Default::default(),
            screen_capture_sources: Default::default(),
            active_cursor: Default::default(),
            pressed_mouse_buttons: Default::default(),
            displays: RefCell::new(vec![TestDisplay::new()]),
            displays_changed_callback: TestDisplayChangeCallback::default(),
            display_generation: Cell::new(1),
            display_snapshot_query_count: Cell::new(0),
            active_window: Default::default(),
            focused_window_available: RefCell::new(true),
            hovered_window_available: RefCell::new(true),
            hovered_window: Default::default(),
            window_stack: Default::default(),
            window_hit_stack: Default::default(),
            native_drag_hysteresis: Default::default(),
            platform_viewport_windows: RefCell::new(true),
            pointer_input_mutation_supported: RefCell::new(true),
            window_creation_capabilities_override: RefCell::new(None),
            window_mutation_capabilities_override: RefCell::new(None),
            next_window_map_error: RefCell::new(None),
            last_created_window: RefCell::new(None),
            next_window_close_during_map: RefCell::new(false),
            next_window_creation_show_fact: RefCell::new(None),
            next_window_initial_presentation_command_outcomes: RefCell::new(None),
            next_window_close_during_initial_presentation: RefCell::new(false),
            next_window_defer_frame_requests: RefCell::new(false),
            expect_restart: Default::default(),
            #[cfg(test)]
            window_appearance_requests: Default::default(),
            #[cfg(test)]
            initial_windowing_requests: Default::default(),
            #[cfg(test)]
            windowing_request_handler: Default::default(),
            idle_sleep_prevention_count: Arc::new(AtomicUsize::new(0)),
            idle_sleep_prevention_delay: Cell::new(Duration::ZERO),
            idle_sleep_prevention_fails: Cell::new(false),
            quit_requested: Default::default(),
            quit_callback: RefCell::new(None),
            open_urls_callback: Default::default(),
            reopen_callback: Default::default(),
            system_notifications: Default::default(),
            system_wake_callback: Default::default(),
            system_sleep_callback: Default::default(),
            app_lifecycle_callback: Default::default(),
            memory_warning_callback: Default::default(),
            thermal_state: Cell::new(ThermalState::Nominal),
            thermal_state_callback: Default::default(),
            power_mode: Cell::new(PowerMode::Unknown),
            power_mode_callback: Default::default(),
            defer_next_launch: Cell::new(false),
            will_open_app_menu_callback: Default::default(),
            app_menu_action_callback: Default::default(),
            validate_app_menu_command_callback: Default::default(),
            menus: Default::default(),
            current_clipboard_item: Mutex::new(None),
            #[cfg(any(test, feature = "test-support"))]
            next_clipboard_read: RefCell::new(None),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            current_primary_item: Mutex::new(None),
            #[cfg(target_os = "macos")]
            current_find_pasteboard_item: Mutex::new(None),
            weak: weak.clone(),
            opened_url: Default::default(),
            text_system,
            headless_renderer_factory,
        })
    }

    pub(crate) fn set_pointer_input_mutation_supported(&self, supported: bool) {
        *self.pointer_input_mutation_supported.borrow_mut() = supported;
    }

    pub(crate) fn connect_display(
        &self,
        display_id: DisplayId,
        refresh_interval: Option<Duration>,
    ) {
        let mut displays = self.displays.borrow_mut();
        if let Some(display) = displays
            .iter_mut()
            .find(|display| display.id() == display_id)
        {
            if display.refresh_interval == refresh_interval {
                return;
            }
            display.refresh_interval = refresh_interval;
        } else {
            let mut display = TestDisplay::with_id(display_id);
            display.refresh_interval = refresh_interval;
            displays.push(display);
        }
        self.advance_display_generation();
    }

    pub(crate) fn simulate_display_added(
        &self,
        display_id: DisplayId,
        refresh_interval: Option<Duration>,
    ) {
        self.connect_display(display_id, refresh_interval);
        self.simulate_displays_changed();
    }

    pub(crate) fn simulate_display_removed(&self, display_id: DisplayId) {
        let mut displays = self.displays.borrow_mut();
        let previous_len = displays.len();
        displays.retain(|display| display.id() != display_id);
        if displays.len() != previous_len {
            self.advance_display_generation();
        }
        drop(displays);
        self.simulate_displays_changed();
    }

    pub(crate) fn simulate_display_refresh_interval_change(
        &self,
        display_id: DisplayId,
        refresh_interval: Option<Duration>,
    ) {
        let mut displays = self.displays.borrow_mut();
        if let Some(display) = displays
            .iter_mut()
            .find(|display| display.id() == display_id)
            && display.refresh_interval != refresh_interval
        {
            display.refresh_interval = refresh_interval;
            self.advance_display_generation();
        }
        drop(displays);
        self.simulate_displays_changed();
    }

    pub(crate) fn simulate_display_bounds_change(
        &self,
        display_id: DisplayId,
        bounds: Bounds<Pixels>,
    ) {
        let mut displays = self.displays.borrow_mut();
        if let Some(display) = displays
            .iter_mut()
            .find(|display| display.id() == display_id)
            && display.bounds != bounds
        {
            display.bounds = bounds;
            self.advance_display_generation();
        }
        drop(displays);
        self.simulate_displays_changed();
    }

    pub(crate) fn simulate_display_properties_change(
        &self,
        display_id: DisplayId,
        bounds: Bounds<Pixels>,
        refresh_interval: Option<Duration>,
    ) {
        let mut displays = self.displays.borrow_mut();
        if let Some(display) = displays
            .iter_mut()
            .find(|display| display.id() == display_id)
            && (display.bounds != bounds || display.refresh_interval != refresh_interval)
        {
            display.bounds = bounds;
            display.refresh_interval = refresh_interval;
            self.advance_display_generation();
        }
        drop(displays);
        self.simulate_displays_changed();
    }

    pub(crate) fn simulate_displays_changed(&self) {
        self.displays_changed_callback.call();
    }

    fn advance_display_generation(&self) {
        self.display_generation.set(
            self.display_generation
                .get()
                .checked_add(1)
                .expect("test display publication generation exhausted"),
        );
    }

    #[cfg(test)]
    pub(crate) fn reset_display_snapshot_query_count(&self) {
        self.display_snapshot_query_count.set(0);
    }

    #[cfg(test)]
    pub(crate) fn display_snapshot_query_count(&self) -> usize {
        self.display_snapshot_query_count.get()
    }

    pub(crate) fn set_window_mutation_capabilities(
        &self,
        capabilities: PlatformWindowMutationCapabilities,
    ) {
        *self.window_mutation_capabilities_override.borrow_mut() = Some(capabilities);
    }

    pub(crate) fn set_window_creation_capabilities(
        &self,
        capabilities: PlatformWindowCreationCapabilities,
    ) {
        *self.window_creation_capabilities_override.borrow_mut() = Some(capabilities);
    }

    pub(crate) fn set_platform_viewport_windows(&self, supported: bool) {
        *self.platform_viewport_windows.borrow_mut() = supported;
    }

    pub(crate) fn set_native_drag_hysteresis(
        &self,
        hysteresis: Option<PlatformNativeDragHysteresis>,
    ) {
        *self.native_drag_hysteresis.borrow_mut() = hysteresis;
    }

    pub(crate) fn fail_next_window_map(&self, message: impl Into<String>) {
        self.next_window_map_error
            .borrow_mut()
            .replace(message.into());
    }

    #[cfg(test)]
    pub(crate) fn last_created_window(&self) -> Option<TestWindow> {
        self.last_created_window.borrow().clone()
    }

    pub(crate) fn close_next_window_during_map(&self) {
        self.next_window_close_during_map.replace(true);
    }

    pub(crate) fn set_next_window_creation_show_fact(&self, show: bool) {
        self.next_window_creation_show_fact
            .borrow_mut()
            .replace(show);
    }

    pub(crate) fn reject_next_window_initial_presentation(&self) {
        self.next_window_initial_presentation_command_outcomes
            .borrow_mut()
            .replace(VecDeque::from([
                PlatformWindowCommandOutcome::Rejected,
                PlatformWindowCommandOutcome::Rejected,
            ]));
    }

    pub(crate) fn close_next_window_during_initial_presentation(&self) {
        self.next_window_close_during_initial_presentation
            .replace(true);
    }

    pub(crate) fn defer_next_window_frame_requests(&self) {
        self.next_window_defer_frame_requests.replace(true);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn simulate_new_path_selection(
        &self,
        select_path: impl FnOnce(&std::path::Path) -> Option<std::path::PathBuf>,
    ) {
        let (path, tx) = self
            .prompts
            .borrow_mut()
            .new_path
            .pop_front()
            .expect("no pending new path prompt");
        tx.send(Ok(select_path(&path))).ok();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn simulate_path_prompt_response(
        &self,
        select_paths: impl FnOnce(&PathPromptOptions) -> Option<Vec<std::path::PathBuf>>,
    ) {
        let (options, tx) = self
            .prompts
            .borrow_mut()
            .paths
            .pop_front()
            .expect("no pending paths prompt");
        let selection = select_paths(&options);
        if let Some(paths) = &selection
            && !options.multiple
            && paths.len() > 1
        {
            panic!(
                "selected {} paths for a prompt that does not allow multiple selection",
                paths.len()
            );
        }
        tx.send(Ok(selection)).ok();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn did_prompt_for_paths(&self) -> bool {
        !self.prompts.borrow().paths.is_empty()
    }

    #[track_caller]
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn simulate_prompt_answer(&self, response: &str) {
        let prompt = self
            .prompts
            .borrow_mut()
            .multiple_choice
            .pop_front()
            .expect("no pending multiple choice prompt");
        let Some(ix) = prompt.answers.iter().position(|a| a == response) else {
            panic!(
                "PROMPT: {}\n{:?}\n{:?}\nCannot respond with {}",
                prompt.msg, prompt.detail, prompt.answers, response
            )
        };
        prompt.tx.send(ix).ok();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn has_pending_prompt(&self) -> bool {
        !self.prompts.borrow().multiple_choice.is_empty()
    }

    pub(crate) fn did_quit(&self) -> bool {
        *self.quit_requested.borrow()
    }

    #[cfg(test)]
    pub(crate) fn take_quit_callback(&self) -> Option<Box<dyn FnMut() -> bool>> {
        self.quit_callback.borrow_mut().take()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn pending_prompt(&self) -> Option<(String, String)> {
        let prompts = self.prompts.borrow();
        let prompt = prompts.multiple_choice.front()?;
        Some((
            prompt.msg.clone(),
            prompt.detail.clone().unwrap_or_default(),
        ))
    }

    pub(crate) fn set_screen_capture_sources(&self, sources: Vec<TestScreenCaptureSource>) {
        *self.screen_capture_sources.borrow_mut() = sources;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn prompt(
        &self,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> oneshot::Receiver<usize> {
        let (tx, rx) = oneshot::channel();
        let answers: Vec<String> = answers.iter().map(|s| s.label().to_string()).collect();
        self.prompts
            .borrow_mut()
            .multiple_choice
            .push_back(TestPrompt {
                msg: msg.to_string(),
                detail: detail.map(|s| s.to_string()),
                answers,
                tx,
            });
        rx
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(crate) fn prompt(
        &self,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> oneshot::Receiver<usize> {
        oneshot::channel().1
    }

    pub(crate) fn set_active_window(&self, window: Option<TestWindow>) {
        let executor = self.foreground_executor();
        let previous_window = self.active_window.borrow_mut().take();
        self.active_window.borrow_mut().clone_from(&window);

        executor
            .spawn(async move {
                if let Some(previous_window) = previous_window {
                    if let Some(window) = window.as_ref()
                        && Rc::ptr_eq(&previous_window.0, &window.0)
                    {
                        return;
                    }
                    previous_window.simulate_active_status_change(false);
                }
                if let Some(window) = window {
                    window.simulate_active_status_change(true);
                }
            })
            .detach();
    }

    pub(crate) fn set_focused_window_available(&self, available: bool) {
        *self.focused_window_available.borrow_mut() = available;
    }

    pub(crate) fn set_hovered_window_available(&self, available: bool) {
        *self.hovered_window_available.borrow_mut() = available;
    }

    pub(crate) fn cursor_style(&self) -> CursorStyle {
        *self.active_cursor.lock()
    }

    pub(crate) fn set_window_cursor_style(&self, window: &TestWindow, style: CursorStyle) {
        let cursor_owner = if *self.hovered_window_available.borrow() {
            self.hovered_window
                .borrow()
                .as_ref()
                .is_some_and(|hovered| Rc::ptr_eq(&hovered.0, &window.0))
        } else {
            self.active_window
                .borrow()
                .as_ref()
                .is_some_and(|active| Rc::ptr_eq(&active.0, &window.0))
        };

        if cursor_owner {
            *self.active_cursor.lock() = style;
        }
    }

    pub(crate) fn set_hovered_window(&self, window: Option<TestWindow>) {
        let previous_window = self.hovered_window.borrow_mut().take();
        self.hovered_window.borrow_mut().clone_from(&window);
        *self.active_cursor.lock() = window
            .as_ref()
            .map(|window| window.0.lock().cursor_style)
            .unwrap_or(CursorStyle::Arrow);

        if let Some(previous_window) = previous_window {
            if let Some(window) = window.as_ref()
                && Rc::ptr_eq(&previous_window.0, &window.0)
            {
                return;
            }
            previous_window.simulate_hover_status_change(false);
        }
        if let Some(window) = window {
            window.simulate_hover_status_change(true);
        }
    }

    pub(crate) fn set_window_stack(&self, windows: Option<Vec<TestWindow>>) {
        *self.window_stack.borrow_mut() = windows;
    }

    pub(crate) fn set_window_hit_stack(&self, stack: PlatformWindowHitStack) {
        self.window_hit_stack.replace(Some(stack));
    }

    pub(crate) fn set_mouse_button_is_pressed(&self, button: MouseButton, pressed: Option<bool>) {
        let mut buttons = self.pressed_mouse_buttons.lock();
        match pressed {
            Some(true) => {
                let buttons = buttons.get_or_insert_with(Vec::new);
                if !buttons.contains(&button) {
                    buttons.push(button);
                }
            }
            Some(false) => {
                let Some(buttons) = buttons.as_mut() else {
                    *buttons = Some(Vec::new());
                    return;
                };
                buttons.retain(|pressed_button| pressed_button != &button);
            }
            None => {
                *buttons = None;
            }
        }
    }

    pub(crate) fn simulate_power_mode_change(&self, mode: PowerMode) {
        self.power_mode.set(mode);
        let callback = self.power_mode_callback.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    #[cfg(test)]
    pub(crate) fn power_mode_notification_handler_for_test(&self) -> NativeAppNotificationHandler {
        self.power_mode_callback
            .borrow()
            .clone()
            .expect("power mode callback must be installed during App initialization")
    }

    pub(crate) fn simulate_thermal_state_change(&self, state: ThermalState) {
        self.thermal_state.set(state);
        let callback = self.thermal_state_callback.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    pub(crate) fn simulate_system_wake(&self) {
        let callback = self
            .system_wake_callback
            .borrow()
            .clone()
            .expect("system wake callback must be installed during App initialization");
        callback();
    }

    pub(crate) fn simulate_system_sleep(&self) {
        let callback = self
            .system_sleep_callback
            .borrow()
            .clone()
            .expect("system sleep callback must be installed during App initialization");
        callback();
    }

    #[cfg(test)]
    pub(crate) fn simulate_app_lifecycle(&self, phase: crate::AppLifecyclePhase) {
        let mut callback = self
            .app_lifecycle_callback
            .take()
            .expect("application lifecycle callback must be installed during App initialization");
        callback(phase);
        let mut slot = self.app_lifecycle_callback.borrow_mut();
        if slot.is_none() {
            *slot = Some(callback);
        }
    }

    #[cfg(test)]
    pub(crate) fn simulate_memory_warning(&self) {
        let mut callback = self
            .memory_warning_callback
            .take()
            .expect("memory warning callback must be installed during App initialization");
        callback();
        let mut slot = self.memory_warning_callback.borrow_mut();
        if slot.is_none() {
            *slot = Some(callback);
        }
    }

    pub(crate) fn simulate_open_urls(&self, urls: Vec<String>) {
        let mut callback = self
            .open_urls_callback
            .take()
            .expect("open URLs callback must be installed during App initialization");
        callback(urls);
        self.open_urls_callback.replace(Some(callback));
    }

    pub(crate) fn simulate_reopen(&self) {
        let mut callback = self
            .reopen_callback
            .take()
            .expect("reopen callback must be installed during App initialization");
        callback();
        self.reopen_callback.replace(Some(callback));
    }

    pub(crate) fn simulate_will_open_app_menu(&self) {
        let mut callback = self
            .will_open_app_menu_callback
            .take()
            .expect("will-open menu callback must be installed during App initialization");
        callback();
        self.will_open_app_menu_callback.replace(Some(callback));
    }

    pub(crate) fn simulate_app_menu_action(&self, action: &dyn crate::Action) {
        let mut callback = self
            .app_menu_action_callback
            .take()
            .expect("application menu callback must be installed during App initialization");
        callback(action);
        self.app_menu_action_callback.replace(Some(callback));
    }

    pub(crate) fn simulate_validate_app_menu_command(&self, action: &dyn crate::Action) -> bool {
        let mut callback = self
            .validate_app_menu_command_callback
            .take()
            .expect("menu validation callback must be installed during App initialization");
        let available = callback(action);
        self.validate_app_menu_command_callback
            .replace(Some(callback));
        available
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn did_prompt_for_new_path(&self) -> bool {
        !self.prompts.borrow().new_path.is_empty()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn active_idle_sleep_preventions(&self) -> usize {
        self.idle_sleep_prevention_count.load(Ordering::SeqCst)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_idle_sleep_prevention_delay(&self, delay: Duration) {
        self.idle_sleep_prevention_delay.set(delay);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_idle_sleep_prevention_fails(&self, fails: bool) {
        self.idle_sleep_prevention_fails.set(fails);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn app_identity(&self) -> Option<(SharedString, SharedString)> {
        self.system_notifications.borrow().app_identity.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn shown_system_notifications(&self) -> Vec<SystemNotification> {
        self.system_notifications.borrow().shown.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn delivered_system_notifications(&self) -> Vec<SystemNotification> {
        self.system_notifications.borrow().delivered.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn dismissed_system_notifications(&self) -> Vec<SharedString> {
        self.system_notifications.borrow().dismissed.clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn simulate_system_notification_response(
        &self,
        response: SystemNotificationResponse,
    ) {
        let callback = self
            .system_notifications
            .borrow_mut()
            .response_callback
            .take();
        if let Some(mut callback) = callback {
            callback(response);
            self.system_notifications
                .borrow_mut()
                .response_callback
                .get_or_insert(callback);
        }
    }
}

impl Platform for TestPlatform {
    fn platform_info(&self) -> crate::PlatformInfo {
        self.platform_info.get()
    }

    fn set_app_identity(&self, identifier: &str, name: &str) {
        self.system_notifications.borrow_mut().app_identity =
            Some((identifier.to_owned().into(), name.to_owned().into()));
    }

    fn show_system_notification(&self, notification: SystemNotification) {
        let mut state = self.system_notifications.borrow_mut();
        if state.app_identity.is_none() {
            return;
        }
        if let Some(delivered) = state
            .delivered
            .iter_mut()
            .find(|delivered| delivered.tag == notification.tag)
        {
            *delivered = notification.clone();
        } else {
            state.delivered.push(notification.clone());
        }
        state.shown.push(notification);
    }

    fn dismiss_system_notification(&self, tag: &str) {
        let mut state = self.system_notifications.borrow_mut();
        state
            .delivered
            .retain(|notification| notification.tag != tag);
        state.dismissed.push(tag.to_owned().into());
    }

    fn on_system_notification_response(
        &self,
        callback: Box<dyn FnMut(SystemNotificationResponse)>,
    ) {
        self.system_notifications.borrow_mut().response_callback = Some(callback);
    }

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
        Box::new(TestKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, _: Box<dyn FnMut()>) {}

    fn on_thermal_state_change(&self, callback: NativeAppNotificationHandler) {
        assert!(
            self.thermal_state_callback
                .replace(Some(callback))
                .is_none(),
            "thermal state callback must be installed exactly once"
        );
    }

    fn power_mode(&self) -> PowerMode {
        self.power_mode.get()
    }

    fn on_power_mode_change(&self, callback: NativeAppNotificationHandler) {
        assert!(
            self.power_mode_callback.replace(Some(callback)).is_none(),
            "power mode callback must be installed exactly once"
        );
    }

    fn on_system_wake(&self, callback: NativeAppNotificationHandler) {
        assert!(
            self.system_wake_callback.replace(Some(callback)).is_none(),
            "system wake callback must be installed exactly once"
        );
    }

    fn on_system_sleep(&self, callback: NativeAppNotificationHandler) {
        assert!(
            self.system_sleep_callback.replace(Some(callback)).is_none(),
            "system sleep callback must be installed exactly once"
        );
    }

    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(crate::AppLifecyclePhase)>) {
        *self.app_lifecycle_callback.borrow_mut() = Some(callback);
    }

    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        *self.memory_warning_callback.borrow_mut() = Some(callback);
    }

    fn thermal_state(&self) -> ThermalState {
        self.thermal_state.get()
    }

    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        let count = self.idle_sleep_prevention_count.clone();
        let fails = self.idle_sleep_prevention_fails.get();
        let reason = reason.to_owned();
        let acquire = move || {
            if fails {
                anyhow::bail!("Idle sleep prevention for {reason:?} is set to fail in this test");
            }
            count.fetch_add(1, Ordering::SeqCst);
            Ok(ActivityGuard::new(move || {
                count.fetch_sub(1, Ordering::SeqCst);
            }))
        };

        let delay = self.idle_sleep_prevention_delay.get();
        if delay.is_zero() {
            Task::ready(acquire())
        } else {
            let delay = self.background_executor.timer(delay);
            self.foreground_executor.spawn(async move {
                delay.await;
                acquire()
            })
        }
    }

    #[cfg(test)]
    fn set_initial_windowing(&self, request: crate::WindowingRequest) {
        self.initial_windowing_requests.borrow_mut().push(request);
    }

    #[cfg(test)]
    fn request_windowing_with_validation(
        &self,
        request: crate::WindowingRequest,
        validate: Box<dyn Fn() -> Result<()>>,
    ) -> Task<Result<()>> {
        if let Err(error) = validate() {
            return Task::ready(Err(error));
        }
        let handler = self.windowing_request_handler.borrow().clone();
        match handler {
            Some(handler) => handler(request, validate),
            None => self.request_windowing(request),
        }
    }

    fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
        if self.defer_next_launch.replace(false) {
            self.foreground_executor
                .spawn(async move { on_finish_launching() })
                .detach();
        } else {
            on_finish_launching();
        }
    }

    fn quit(&self) {
        self.quit_requested.replace(true);
    }

    fn restart(&self, path: Option<PathBuf>, arguments: Vec<OsString>) {
        if let Some(tx) = self.expect_restart.take() {
            tx.send((path, arguments)).unwrap();
        }
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        //
    }

    fn hide(&self) {
        unimplemented!()
    }

    fn hide_other_apps(&self) {
        unimplemented!()
    }

    fn unhide_other_apps(&self) {
        unimplemented!()
    }

    fn displays(&self) -> Vec<std::rc::Rc<dyn crate::PlatformDisplay>> {
        self.displays
            .borrow()
            .iter()
            .map(|display| Rc::new(display.clone()) as Rc<dyn PlatformDisplay>)
            .collect()
    }

    fn primary_display(&self) -> Option<std::rc::Rc<dyn crate::PlatformDisplay>> {
        self.displays
            .borrow()
            .first()
            .map(|display| Rc::new(display.clone()) as Rc<dyn PlatformDisplay>)
    }

    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.displays_changed_callback.set(callback);
    }

    fn display_snapshot(&self) -> PlatformDisplaySnapshot {
        self.display_snapshot_query_count
            .set(self.display_snapshot_query_count.get() + 1);
        let displays = self.displays();
        let primary = displays.first().map(|display| display.id());
        PlatformDisplaySnapshot::try_new(Some(self.display_generation.get()), displays, primary)
            .expect("test platform display publication must be valid")
    }

    fn is_screen_capture_supported(&self) -> bool {
        true
    }

    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        let (mut tx, rx) = oneshot::channel();
        tx.send(Ok(self
            .screen_capture_sources
            .borrow()
            .iter()
            .map(|source| Rc::new(source.clone()) as Rc<dyn ScreenCaptureSource>)
            .collect()))
            .ok();
        rx
    }

    fn active_window(&self) -> Option<crate::AnyWindowHandle> {
        self.active_window
            .borrow()
            .as_ref()
            .map(|window| window.0.lock().handle)
    }

    fn focused_window(&self) -> PlatformFocusedWindow {
        if *self.focused_window_available.borrow() {
            PlatformFocusedWindow::from_window(self.active_window())
        } else {
            PlatformFocusedWindow::Unavailable
        }
    }

    fn hovered_window(&self) -> PlatformHoveredWindow {
        if !*self.hovered_window_available.borrow() {
            return PlatformHoveredWindow::Unavailable;
        }
        PlatformHoveredWindow::from_window(
            self.hovered_window
                .borrow()
                .as_ref()
                .map(|window| window.0.lock().handle),
        )
    }

    fn window_stack(&self) -> Option<Vec<crate::AnyWindowHandle>> {
        self.window_stack.borrow().as_ref().map(|windows| {
            windows
                .iter()
                .map(|window| window.0.lock().handle)
                .collect()
        })
    }

    fn window_hit_stack_at(&self, point: Point<DevicePixels>) -> PlatformWindowHitStack {
        self.window_hit_stack
            .borrow()
            .as_ref()
            .filter(|stack| {
                stack
                    .observation()
                    .is_some_and(|observation| observation.sampled_point() == point)
            })
            .cloned()
            .unwrap_or(PlatformWindowHitStack::Unavailable)
    }

    fn native_drag_hysteresis(&self) -> Option<PlatformNativeDragHysteresis> {
        *self.native_drag_hysteresis.borrow()
    }

    fn viewport_capabilities(&self) -> crate::PlatformViewportCapabilities {
        crate::PlatformViewportCapabilities {
            platform_viewport_windows: *self.platform_viewport_windows.borrow(),
            global_window_bounds: true,
            window_stack: self.window_stack.borrow().is_some(),
            window_hit_stack: self
                .window_hit_stack
                .borrow()
                .as_ref()
                .is_some_and(|stack| stack.observation().is_some()),
            display_work_area: true,
            dpi_scale: true,
            hovered_window_ignores_no_input: true,
            ..Default::default()
        }
    }

    fn window_capabilities(
        &self,
        _kind: &crate::WindowKind,
        _display_id: Option<crate::DisplayId>,
    ) -> PlatformWindowCapabilities {
        let mutations = self
            .window_mutation_capabilities_override
            .borrow()
            .unwrap_or(PlatformWindowMutationCapabilities {
                position: WindowMutationSupport::Live,
                physical_placement: WindowMutationSupport::Live,
                size: WindowMutationSupport::Live,
                windowed: WindowMutationSupport::Live,
                maximized: WindowMutationSupport::Live,
                fullscreen: WindowMutationSupport::Live,
                minimized: WindowMutationSupport::Live,
                restore_bounds: WindowMutationSupport::Live,
                pointer_input: if *self.pointer_input_mutation_supported.borrow() {
                    WindowMutationSupport::Live
                } else {
                    WindowMutationSupport::Unsupported
                },
                activation_policy: WindowMutationSupport::Live,
                alpha: WindowMutationSupport::CreationOnly,
                topmost: WindowMutationSupport::Unsupported,
                taskbar_visibility: WindowMutationSupport::Unsupported,
                coordinate_space: WindowCoordinateSpace::GlobalScreen,
            });
        PlatformWindowCapabilities {
            creation: self
                .window_creation_capabilities_override
                .borrow()
                .unwrap_or(PlatformWindowCreationCapabilities {
                    focus_on_appearing: WindowCreationSupport::Supported,
                    transient_for: WindowCreationSupport::Supported,
                    provisional_presentation: WindowCreationSupport::Supported,
                    initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
                }),
            mutations,
            activation: PlatformWindowActivationSupport::Observed,
        }
    }

    fn mouse_button_is_pressed(&self, button: MouseButton) -> Option<bool> {
        self.pressed_mouse_buttons
            .lock()
            .as_ref()
            .map(|buttons| buttons.contains(&button))
    }

    fn validate_window_kind(&self, kind: &crate::WindowKind) -> anyhow::Result<()> {
        if matches!(kind, crate::WindowKind::AnchoredPopup(_)) {
            return Err(crate::popup::PopupNotSupportedError.into());
        }
        Ok(())
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> anyhow::Result<Box<dyn crate::PlatformWindow>> {
        self.validate_window_kind(&params.kind)?;
        let renderer = match self.headless_renderer_factory.as_ref() {
            Some(factory) => factory()?,
            None => None,
        };
        let display = params
            .display_id
            .and_then(|id| {
                self.displays()
                    .into_iter()
                    .find(|display| display.id() == id)
            })
            .or_else(|| self.primary_display());
        let window = TestWindow::new(
            handle,
            params,
            self.weak.clone(),
            display,
            renderer,
            self.next_window_map_error.borrow_mut().take(),
            self.next_window_close_during_map.replace(false),
            self.next_window_creation_show_fact.borrow_mut().take(),
            self.next_window_initial_presentation_command_outcomes
                .borrow_mut()
                .take(),
            self.next_window_close_during_initial_presentation
                .replace(false),
            self.next_window_defer_frame_requests.replace(false),
        );
        self.last_created_window
            .borrow_mut()
            .replace(window.clone());
        Ok(Box::new(window))
    }

    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Light
    }

    #[cfg(test)]
    fn set_window_appearance(&self, appearance: Option<WindowAppearance>) {
        self.window_appearance_requests
            .borrow_mut()
            .push(appearance);
    }

    fn open_url(&self, url: &str) {
        *self.opened_url.borrow_mut() = Some(url.to_string())
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        assert!(
            self.open_urls_callback.replace(Some(callback)).is_none(),
            "open URLs callback must be installed exactly once"
        );
    }

    #[cfg(any(test, feature = "test-support"))]
    fn prompt_for_paths(
        &self,
        options: crate::PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<std::path::PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        self.prompts.borrow_mut().paths.push_back((options, tx));
        rx
    }

    #[cfg(any(test, feature = "test-support"))]
    fn prompt_for_new_path(
        &self,
        directory: &std::path::Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<std::path::PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        self.prompts
            .borrow_mut()
            .new_path
            .push_back((directory.to_path_buf(), tx));
        rx
    }

    #[cfg(not(any(test, feature = "test-support")))]
    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        oneshot::channel().1
    }

    #[cfg(not(any(test, feature = "test-support")))]
    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        oneshot::channel().1
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        true
    }

    fn reveal_path(&self, _path: &std::path::Path) {
        unimplemented!()
    }

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.quit_callback.replace(Some(callback));
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        assert!(
            self.reopen_callback.replace(Some(callback)).is_none(),
            "reopen callback must be installed exactly once"
        );
    }

    fn set_menus(&self, menus: Vec<crate::Menu>, _keymap: &Keymap) {
        *self.menus.borrow_mut() = menus.into_iter().map(|menu| menu.owned()).collect();
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        Some(self.menus.borrow().clone())
    }
    fn set_dock_menu(&self, _menu: Vec<crate::MenuItem>, _keymap: &Keymap) {}

    fn add_recent_document(&self, _paths: &Path) {}

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn crate::Action)>) {
        assert!(
            self.app_menu_action_callback
                .replace(Some(callback))
                .is_none(),
            "application menu callback must be installed exactly once"
        );
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        assert!(
            self.will_open_app_menu_callback
                .replace(Some(callback))
                .is_none(),
            "will-open menu callback must be installed exactly once"
        );
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn crate::Action) -> bool>) {
        assert!(
            self.validate_app_menu_command_callback
                .replace(Some(callback))
                .is_none(),
            "menu validation callback must be installed exactly once"
        );
    }

    fn app_path(&self) -> Result<std::path::PathBuf> {
        unimplemented!()
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<std::path::PathBuf> {
        unimplemented!()
    }

    fn hide_cursor_until_mouse_moves(&self) {}

    fn is_cursor_visible(&self) -> bool {
        true
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        false
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.current_clipboard_item.lock().clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn read_from_clipboard_async(&self) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        self.next_clipboard_read
            .borrow_mut()
            .take()
            .unwrap_or_else(|| Task::ready(Ok(self.read_from_clipboard())))
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        *self.current_clipboard_item.lock() = Some(item);
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.current_primary_item.lock().clone()
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem) {
        *self.current_primary_item.lock() = Some(item);
    }

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.current_find_pasteboard_item.lock().clone()
    }

    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        *self.current_find_pasteboard_item.lock() = Some(item);
    }

    fn write_credentials(&self, _url: &str, _username: &str, _password: &[u8]) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn read_credentials(&self, _url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn register_url_scheme(&self, _: &str) -> Task<anyhow::Result<()>> {
        unimplemented!()
    }

    fn open_with_system(&self, _path: &Path) {
        unimplemented!()
    }
}

impl TestScreenCaptureSource {
    /// Create a fake screen capture source, for testing.
    pub fn new() -> Self {
        Self {}
    }
}

struct TestKeyboardLayout;

impl PlatformKeyboardLayout for TestKeyboardLayout {
    fn id(&self) -> &str {
        "open-gpui.keyboard.example"
    }

    fn name(&self) -> &str {
        "open-gpui.keyboard.example"
    }
}

#[cfg(test)]
mod platform_info_tests {
    use super::*;
    use crate::{OperatingSystem, PlatformInfo, PlatformTarget, TestDispatcher};

    #[test]
    fn default_identity_matches_native_host_or_unknown_browser() {
        let dispatcher = Arc::new(TestDispatcher::new(0));
        let platform = TestPlatform::new(
            BackgroundExecutor::new(dispatcher.clone()),
            ForegroundExecutor::new(dispatcher),
        );
        let expected = if cfg!(target_family = "wasm") {
            PlatformInfo {
                os: OperatingSystem::Unknown,
                target: PlatformTarget::Web,
            }
        } else {
            PlatformInfo {
                os: match std::env::consts::OS {
                    "macos" => OperatingSystem::Mac,
                    "windows" => OperatingSystem::Windows,
                    "linux" => OperatingSystem::Linux,
                    "freebsd" => OperatingSystem::FreeBsd,
                    "ios" => OperatingSystem::Ios,
                    "android" => OperatingSystem::Android,
                    os => panic!("unsupported test host {os}"),
                },
                target: PlatformTarget::Native,
            }
        };
        assert_eq!(platform.platform_info(), expected);

        let backend: Rc<dyn Platform> = platform.clone();
        for os in [
            OperatingSystem::Mac,
            OperatingSystem::Windows,
            OperatingSystem::Linux,
            OperatingSystem::FreeBsd,
            OperatingSystem::Ios,
            OperatingSystem::Android,
            OperatingSystem::Unknown,
        ] {
            for target in [PlatformTarget::Native, PlatformTarget::Web] {
                let identity = PlatformInfo { os, target };
                platform.platform_info.set(identity);
                assert_eq!(backend.platform_info(), identity);
            }
        }
    }
}
