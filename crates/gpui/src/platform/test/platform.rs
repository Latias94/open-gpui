use crate::{
    AnyWindowHandle, BackgroundExecutor, ClipboardItem, CursorStyle, DevicePixels,
    DummyKeyboardMapper, ForegroundExecutor, Keymap, MouseButton, NoopTextSystem,
    PathPromptOptions, Platform, PlatformDisplay, PlatformDisplaySnapshot, PlatformFocusedWindow,
    PlatformHeadlessRenderer, PlatformHoveredWindow, PlatformKeyboardLayout,
    PlatformKeyboardMapper, PlatformNativeDragHysteresis, PlatformTextSystem,
    PlatformWindowCapabilities, PlatformWindowCommandOutcome, PlatformWindowCreationCapabilities,
    PlatformWindowHitStack, PlatformWindowMutationCapabilities, Point, PromptButton,
    ScreenCaptureFrame, ScreenCaptureSource, ScreenCaptureStream, SourceMetadata, Task,
    TestDisplay, TestWindow, ThermalState, WindowAppearance, WindowCoordinateSpace,
    WindowCreationSupport, WindowInitialPresentationOrder, WindowMutationSupport, WindowParams,
    size,
};
use anyhow::Result;
use futures::channel::oneshot;
use open_gpui_collections::VecDeque;
use parking_lot::Mutex;
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::Arc,
};

/// TestPlatform implements the Platform trait for use in tests.
pub(crate) struct TestPlatform {
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
    active_display: Rc<dyn PlatformDisplay>,
    display_snapshot_query_count: Cell<usize>,
    active_cursor: Mutex<CursorStyle>,
    pressed_mouse_buttons: Mutex<Option<Vec<MouseButton>>>,
    current_clipboard_item: Mutex<Option<ClipboardItem>>,
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    current_primary_item: Mutex<Option<ClipboardItem>>,
    #[cfg(target_os = "macos")]
    current_find_pasteboard_item: Mutex<Option<ClipboardItem>>,
    pub(crate) prompts: RefCell<TestPrompts>,
    screen_capture_sources: RefCell<Vec<TestScreenCaptureSource>>,
    pub opened_url: RefCell<Option<String>>,
    pub text_system: Arc<dyn PlatformTextSystem>,
    pub expect_restart: RefCell<Option<oneshot::Sender<Option<PathBuf>>>>,
    quit_requested: RefCell<bool>,
    open_urls_callback: RefCell<Option<Box<dyn FnMut(Vec<String>)>>>,
    reopen_callback: RefCell<Option<Box<dyn FnMut()>>>,
    system_wake_callback: RefCell<Option<Box<dyn FnMut()>>>,
    will_open_app_menu_callback: RefCell<Option<Box<dyn FnMut()>>>,
    app_menu_action_callback: RefCell<Option<Box<dyn FnMut(&dyn crate::Action)>>>,
    validate_app_menu_command_callback: RefCell<Option<Box<dyn FnMut(&dyn crate::Action) -> bool>>>,
    headless_renderer_factory: Option<Box<dyn Fn() -> Option<Box<dyn PlatformHeadlessRenderer>>>>,
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

struct TestPrompt {
    msg: String,
    detail: Option<String>,
    answers: Vec<String>,
    tx: oneshot::Sender<usize>,
}

#[derive(Default)]
pub(crate) struct TestPrompts {
    multiple_choice: VecDeque<TestPrompt>,
    new_path: VecDeque<(PathBuf, oneshot::Sender<Result<Option<PathBuf>>>)>,
    paths: VecDeque<(
        PathPromptOptions,
        oneshot::Sender<Result<Option<Vec<PathBuf>>>>,
    )>,
}

impl TestPlatform {
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
            Box<dyn Fn() -> Option<Box<dyn PlatformHeadlessRenderer>>>,
        >,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak| TestPlatform {
            background_executor: executor,
            foreground_executor,
            prompts: Default::default(),
            screen_capture_sources: Default::default(),
            active_cursor: Default::default(),
            pressed_mouse_buttons: Default::default(),
            active_display: Rc::new(TestDisplay::new()),
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
            quit_requested: Default::default(),
            open_urls_callback: Default::default(),
            reopen_callback: Default::default(),
            system_wake_callback: Default::default(),
            will_open_app_menu_callback: Default::default(),
            app_menu_action_callback: Default::default(),
            validate_app_menu_command_callback: Default::default(),
            current_clipboard_item: Mutex::new(None),
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

    pub(crate) fn did_prompt_for_paths(&self) -> bool {
        !self.prompts.borrow().paths.is_empty()
    }

    #[track_caller]
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

    pub(crate) fn has_pending_prompt(&self) -> bool {
        !self.prompts.borrow().multiple_choice.is_empty()
    }

    pub(crate) fn did_quit(&self) -> bool {
        *self.quit_requested.borrow()
    }

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

    pub(crate) fn simulate_system_wake(&self) {
        let mut callback = self
            .system_wake_callback
            .take()
            .expect("system wake callback must be installed during App initialization");
        callback();
        self.system_wake_callback.replace(Some(callback));
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

    pub(crate) fn did_prompt_for_new_path(&self) -> bool {
        !self.prompts.borrow().new_path.is_empty()
    }
}

impl Platform for TestPlatform {
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

    fn on_thermal_state_change(&self, _: Box<dyn FnMut()>) {}

    fn on_system_wake(&self, callback: Box<dyn FnMut()>) {
        assert!(
            self.system_wake_callback.replace(Some(callback)).is_none(),
            "system wake callback must be installed exactly once"
        );
    }

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn run(&self, _on_finish_launching: Box<dyn FnOnce()>) {
        unimplemented!()
    }

    fn quit(&self) {
        self.quit_requested.replace(true);
    }

    fn restart(&self, path: Option<PathBuf>) {
        if let Some(tx) = self.expect_restart.take() {
            tx.send(path).unwrap();
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
        vec![self.active_display.clone()]
    }

    fn primary_display(&self) -> Option<std::rc::Rc<dyn crate::PlatformDisplay>> {
        Some(self.active_display.clone())
    }

    fn display_snapshot(&self) -> PlatformDisplaySnapshot {
        self.display_snapshot_query_count
            .set(self.display_snapshot_query_count.get() + 1);
        PlatformDisplaySnapshot::try_new(
            Some(1),
            vec![self.active_display.clone()],
            Some(self.active_display.id()),
        )
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
        }
    }

    fn mouse_button_is_pressed(&self, button: MouseButton) -> Option<bool> {
        self.pressed_mouse_buttons
            .lock()
            .as_ref()
            .map(|buttons| buttons.contains(&button))
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> anyhow::Result<Box<dyn crate::PlatformWindow>> {
        let renderer = self.headless_renderer_factory.as_ref().and_then(|f| f());
        let window = TestWindow::new(
            handle,
            params,
            self.weak.clone(),
            self.active_display.clone(),
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

    fn open_url(&self, url: &str) {
        *self.opened_url.borrow_mut() = Some(url.to_string())
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        assert!(
            self.open_urls_callback.replace(Some(callback)).is_none(),
            "open URLs callback must be installed exactly once"
        );
    }

    fn prompt_for_paths(
        &self,
        options: crate::PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<std::path::PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        self.prompts.borrow_mut().paths.push_back((options, tx));
        rx
    }

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

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        true
    }

    fn reveal_path(&self, _path: &std::path::Path) {
        unimplemented!()
    }

    fn on_quit(&self, _callback: Box<dyn FnMut()>) {}

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        assert!(
            self.reopen_callback.replace(Some(callback)).is_none(),
            "reopen callback must be installed exactly once"
        );
    }

    fn set_menus(&self, _menus: Vec<crate::Menu>, _keymap: &Keymap) {}
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
