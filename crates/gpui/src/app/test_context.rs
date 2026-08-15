use crate::{
    Action, AnyView, AnyWindowHandle, App, AppCell, AppContext, AsyncApp, AvailableSpace,
    BackgroundExecutor, BorrowAppContext, Bounds, Capslock, ClipboardItem, CursorStyle,
    DispatchEventResult, DrawPhase, Drawable, Element, Empty, EntityId, EventEmitter,
    ForegroundExecutor, Global, InputEvent, Keystroke, Modifiers, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent, Pixels, Platform,
    PlatformPointerCaptureReleaseOutcome, PlatformWindowCreationCapabilities,
    PlatformWindowDispatch, PlatformWindowHitStack, PlatformWindowMutationCapabilities,
    PlatformWindowMutationTerminal, PlatformWindowPresentOutcome, Point, Render, Result, Size,
    Task, TestDispatcher, TestPlatform, TestScreenCaptureSource, TestWindow, TextSystem,
    VisualContext, Window, WindowBounds, WindowHandle, WindowMutationDomain, WindowOptions,
    WindowPlatformFacts, app::GpuiMode, platform::RequestFrameOptions, window::ElementArenaScope,
};
use anyhow::{anyhow, bail};
use futures::{Stream, StreamExt, channel::oneshot};

use std::{
    cell::RefCell,
    future::Future,
    ops::{Deref, Range},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

/// Stable test-facing summary for the most recent simulated platform input dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestInputDispatchSnapshot {
    propagated: bool,
    default_prevented: bool,
}

impl Default for TestInputDispatchSnapshot {
    fn default() -> Self {
        Self {
            propagated: true,
            default_prevented: false,
        }
    }
}

impl TestInputDispatchSnapshot {
    /// Returns whether event propagation was still enabled when dispatch finished.
    pub const fn propagated(&self) -> bool {
        self.propagated
    }

    /// Returns whether default behavior was prevented while dispatching the input.
    pub const fn default_prevented(&self) -> bool {
        self.default_prevented
    }

    /// Returns whether GPUI consumed the platform default behavior for the input.
    pub const fn default_consumed(&self) -> bool {
        self.default_prevented
    }

    /// Returns whether propagation was stopped by an input handler.
    pub const fn propagation_stopped(&self) -> bool {
        !self.propagated
    }
}

impl From<DispatchEventResult> for TestInputDispatchSnapshot {
    fn from(value: DispatchEventResult) -> Self {
        Self {
            propagated: value.propagate,
            default_prevented: value.default_prevented,
        }
    }
}

/// Stable test-facing outcome for a simulated platform close request.
///
/// A native platform can be vetoed while a higher-level owner continues a
/// coordinated shutdown. The outcome deliberately exposes only facts owned by the
/// generic platform boundary; component-specific session acceptance remains the
/// responsibility of that component's state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestWindowCloseRequestOutcome {
    native_close_allowed: bool,
    logical_window_removed: bool,
    native_terminal_started: bool,
}

impl TestWindowCloseRequestOutcome {
    /// Returns whether the synchronous native close callback allowed immediate native terminal.
    pub const fn native_close_allowed(&self) -> bool {
        self.native_close_allowed
    }

    /// Returns whether the logical window was removed from the App registry.
    pub const fn logical_window_removed(&self) -> bool {
        self.logical_window_removed
    }

    /// Returns whether the test platform observed the native window as terminal.
    pub const fn native_terminal_started(&self) -> bool {
        self.native_terminal_started
    }

    /// Returns whether the request began any terminal transition observable by the test platform.
    pub const fn terminal_transition_started(&self) -> bool {
        self.logical_window_removed || self.native_terminal_started
    }
}

/// A retained read-only view of one TestPlatform window's activation command count.
///
/// The probe remains readable after the logical window leaves the App registry, which lets
/// shutdown tests verify that no late activation escaped before native terminal convergence.
#[derive(Clone)]
pub struct TestWindowActivationProbe {
    window: TestWindow,
}

impl TestWindowActivationProbe {
    /// Returns how many native activation commands the test window has accepted.
    pub fn count(&self) -> usize {
        self.window.activation_count()
    }
}

/// Holds a TestPlatform native close callback after GPUI has removed the logical window.
///
/// Call [`Self::release`] to deliver the terminal native `Closed` event at a controlled point.
/// Dropping the hold also releases the callback so a failed test cannot retain terminal work.
pub struct TestWindowNativeTerminalHold {
    window: Option<TestWindow>,
}

impl TestWindowNativeTerminalHold {
    /// Delivers the held native `Closed` event.
    pub fn release(mut self) -> bool {
        self.window
            .take()
            .is_some_and(|window| window.release_deferred_native_terminal())
    }
}

impl Drop for TestWindowNativeTerminalHold {
    fn drop(&mut self) {
        if let Some(window) = self.window.take() {
            let _ = window.release_deferred_native_terminal();
        }
    }
}

/// A TestAppContext is provided to tests created with `#[open_gpui::test]`, it provides
/// an implementation of `Context` with additional methods that are useful in tests.
#[derive(Clone)]
pub struct TestAppContext {
    #[doc(hidden)]
    pub background_executor: BackgroundExecutor,
    #[doc(hidden)]
    pub foreground_executor: ForegroundExecutor,
    #[doc(hidden)]
    pub dispatcher: TestDispatcher,
    test_platform: Rc<TestPlatform>,
    text_system: Arc<TextSystem>,
    fn_name: Option<&'static str>,
    on_quit: Rc<RefCell<Vec<Box<dyn FnOnce() + 'static>>>>,
    last_input_dispatch: Rc<RefCell<Option<TestInputDispatchSnapshot>>>,
    #[doc(hidden)]
    pub app: Rc<AppCell>,
}

impl AppContext for TestAppContext {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.new(build_entity)
    }

    fn reserve_entity<T: 'static>(&mut self) -> crate::Reservation<T> {
        let mut app = self.app.borrow_mut();
        app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: crate::Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.insert_entity(reservation, build_entity)
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let mut app = self.app.borrow_mut();
        app.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> super::GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut with a test app context. Try calling update() first")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let app = self.app.borrow();
        app.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let mut lock = self.app.borrow_mut();
        lock.update_window(window, f)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        let mut lock = self.app.borrow_mut();
        lock.with_window(entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        let app = self.app.borrow();
        app.read_window(window, read)
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        let app = self.app.borrow();
        app.read_global(callback)
    }
}

impl TestAppContext {
    /// Creates a new `TestAppContext`. Usually you can rely on `#[open_gpui::test]` to do this for you.
    pub fn build(dispatcher: TestDispatcher, fn_name: Option<&'static str>) -> Self {
        let arc_dispatcher = Arc::new(dispatcher.clone());
        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);
        let platform = TestPlatform::new(background_executor.clone(), foreground_executor.clone());
        let asset_source = Arc::new(());
        let http_client = open_gpui_http_client::FakeHttpClient::with_404_response();
        let text_system = Arc::new(TextSystem::new(platform.text_system()));

        let app = App::new_app(platform.clone(), asset_source, http_client);
        app.borrow_mut().mode = GpuiMode::test();

        Self {
            app,
            background_executor,
            foreground_executor,
            dispatcher,
            test_platform: platform,
            text_system,
            fn_name,
            on_quit: Rc::new(RefCell::new(Vec::default())),
            last_input_dispatch: Rc::new(RefCell::new(None)),
        }
    }

    /// Skip all drawing operations for the duration of this test.
    pub fn skip_drawing(&mut self) {
        self.app.borrow_mut().mode = GpuiMode::Test { skip_drawing: true };
    }

    /// Create a single TestAppContext, for non-multi-client tests
    pub fn single() -> Self {
        let dispatcher = TestDispatcher::new(0);
        Self::build(dispatcher, None)
    }

    /// The name of the test function that created this `TestAppContext`
    pub fn test_function_name(&self) -> Option<&'static str> {
        self.fn_name
    }

    /// Checks whether there have been any new path prompts received by the platform.
    pub fn did_prompt_for_new_path(&self) -> bool {
        self.test_platform.did_prompt_for_new_path()
    }

    /// returns a new `TestAppContext` re-using the same executors to interleave tasks.
    pub fn new_app(&self) -> TestAppContext {
        Self::build(self.dispatcher.clone(), self.fn_name)
    }

    /// Called by the test helper to end the test.
    /// public so the macro can call it.
    pub fn quit(&self) {
        self.on_quit.borrow_mut().drain(..).for_each(|f| f());
        self.app.borrow_mut().shutdown();
    }

    /// Register cleanup to run when the test ends.
    pub fn on_quit(&mut self, f: impl FnOnce() + 'static) {
        self.on_quit.borrow_mut().push(Box::new(f));
    }

    /// Schedules all windows to be redrawn on the next effect cycle.
    pub fn refresh(&mut self) -> Result<()> {
        let mut app = self.app.borrow_mut();
        app.refresh_windows();
        Ok(())
    }

    /// Returns an executor (for running tasks in the background)
    pub fn executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    /// Returns an executor (for running tasks on the main thread)
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }

    /// Gives you an `&mut App` for the duration of the closure
    pub fn update<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        let mut cx = self.app.borrow_mut();
        cx.update(f)
    }

    /// Gives you an `&App` for the duration of the closure
    pub fn read<R>(&self, f: impl FnOnce(&App) -> R) -> R {
        let cx = self.app.borrow();
        f(&cx)
    }

    /// Returns the most recent simulated platform input dispatch summary.
    pub fn last_input_dispatch(&self) -> Option<TestInputDispatchSnapshot> {
        *self.last_input_dispatch.borrow()
    }

    /// Clears the most recent simulated platform input dispatch summary.
    pub fn clear_last_input_dispatch(&self) {
        self.last_input_dispatch.replace(None);
    }

    pub(crate) fn record_input_dispatch(
        &self,
        result: DispatchEventResult,
    ) -> TestInputDispatchSnapshot {
        let snapshot = TestInputDispatchSnapshot::from(result);
        self.last_input_dispatch.replace(Some(snapshot));
        snapshot
    }

    /// Adds a new window. The Window will always be backed by a `TestWindow` which
    /// can be retrieved with `self.test_window(handle)`
    pub fn add_window<F, V>(&mut self, build_window: F) -> WindowHandle<V>
    where
        F: FnOnce(&mut Window, &mut Context<V>) -> V,
        V: 'static + Render,
    {
        let mut cx = self.app.borrow_mut();

        // Some tests rely on the window size matching the bounds of the test display
        let bounds = Bounds::maximized(None, &cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus_on_appearing: false,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| build_window(window, cx)),
        )
        .unwrap()
    }

    /// Opens a new window with a specific size.
    ///
    /// Unlike `add_window` which uses maximized bounds, this allows controlling
    /// the window dimensions, which is important for layout-sensitive tests.
    pub fn open_window<F, V>(
        &mut self,
        window_size: Size<Pixels>,
        build_window: F,
    ) -> WindowHandle<V>
    where
        F: FnOnce(&mut Window, &mut Context<V>) -> V,
        V: 'static + Render,
    {
        let mut cx = self.app.borrow_mut();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: window_size,
                })),
                focus_on_appearing: false,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| build_window(window, cx)),
        )
        .unwrap()
    }

    /// Adds a new window with no content.
    pub fn add_empty_window(&mut self) -> &mut VisualTestContext {
        let mut cx = self.app.borrow_mut();
        let bounds = Bounds::maximized(None, &cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Empty),
            )
            .unwrap();
        drop(cx);
        let cx = VisualTestContext::from_window(*window.deref(), self).into_mut();
        cx.run_until_parked();
        cx
    }

    /// Adds a new window, and returns its root view and a `VisualTestContext` which can be used
    /// as a `Window` and `App` for the rest of the test. Typically you would shadow this context with
    /// the returned one. `let (view, cx) = cx.add_window_view(...);`
    pub fn add_window_view<F, V>(
        &mut self,
        build_root_view: F,
    ) -> (Entity<V>, &mut VisualTestContext)
    where
        F: FnOnce(&mut Window, &mut Context<V>) -> V,
        V: 'static + Render,
    {
        let mut cx = self.app.borrow_mut();
        let bounds = Bounds::maximized(None, &cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus_on_appearing: false,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| build_root_view(window, cx)),
            )
            .unwrap();
        drop(cx);
        let view = window.root(self).unwrap();
        let cx = VisualTestContext::from_window(*window.deref(), self).into_mut();
        cx.run_until_parked();

        // it might be nice to try and cleanup these at the end of each test.
        (view, cx)
    }

    /// returns the TextSystem
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// Simulates writing to the platform clipboard
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.test_platform.write_to_clipboard(item)
    }

    /// Simulates reading from the platform clipboard.
    /// This will return the most recent value from `write_to_clipboard`.
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.test_platform.read_from_clipboard()
    }

    /// Simulates choosing a File in the platform's "Open" dialog.
    pub fn simulate_new_path_selection(
        &self,
        select_path: impl FnOnce(&std::path::Path) -> Option<std::path::PathBuf>,
    ) {
        self.test_platform.simulate_new_path_selection(select_path);
    }

    /// Simulates responding to a `prompt_for_paths` ("Open") dialog.
    pub fn simulate_path_prompt_response(
        &self,
        select_paths: impl FnOnce(&crate::PathPromptOptions) -> Option<Vec<std::path::PathBuf>>,
    ) {
        self.test_platform
            .simulate_path_prompt_response(select_paths);
    }

    /// Returns true if there's a path selection dialog pending.
    pub fn did_prompt_for_paths(&self) -> bool {
        self.test_platform.did_prompt_for_paths()
    }

    /// Simulates clicking a button in an platform-level alert dialog.
    #[track_caller]
    pub fn simulate_prompt_answer(&self, button: &str) {
        self.test_platform.simulate_prompt_answer(button);
    }

    /// Returns true if there's an alert dialog open.
    pub fn has_pending_prompt(&self) -> bool {
        self.test_platform.has_pending_prompt()
    }

    /// Returns true if there's an alert dialog open.
    pub fn pending_prompt(&self) -> Option<(String, String)> {
        self.test_platform.pending_prompt()
    }

    /// All the urls that have been opened with cx.open_url() during this test.
    pub fn opened_url(&self) -> Option<String> {
        self.test_platform.opened_url.borrow().clone()
    }

    /// Simulates the user resizing the window to the new size.
    pub fn simulate_window_resize(&self, window_handle: AnyWindowHandle, size: Size<Pixels>) {
        self.test_window(window_handle).simulate_resize(size);
        self.run_until_parked();
    }

    /// Simulates the platform minimizing a window and publishing coherent window facts.
    pub fn simulate_window_minimize(&self, window_handle: AnyWindowHandle) {
        self.test_window(window_handle).simulate_minimize();
        self.run_until_parked();
    }

    /// Configures the next structured placement dispatch from a test window.
    pub fn set_next_window_placement_dispatch(
        &self,
        window: AnyWindowHandle,
        dispatch: PlatformWindowDispatch,
    ) {
        self.test_window(window)
            .set_next_placement_dispatch(dispatch);
    }

    /// Configures the next pointer-input dispatch from a test window.
    pub fn set_next_window_pointer_input_dispatch(
        &self,
        window: AnyWindowHandle,
        dispatch: PlatformWindowDispatch,
    ) {
        self.test_window(window)
            .set_next_pointer_input_dispatch(dispatch);
    }

    /// Configures the renderer outcome returned by a test window's presentation attempts.
    pub fn set_window_present_outcome(
        &self,
        window: AnyWindowHandle,
        outcome: PlatformWindowPresentOutcome,
    ) {
        self.test_window(window).set_present_outcome(outcome);
    }

    /// Applies the latest queued mutation in one domain and emits its observed terminal facts.
    pub fn flush_window_mutation(
        &self,
        window: AnyWindowHandle,
        domain: WindowMutationDomain,
    ) -> bool {
        let emitted = self.test_window(window).flush_window_mutation(domain);
        self.run_until_parked();
        emitted
    }

    /// Emits supplied backend facts as an observed terminal result for one mutation domain.
    pub fn simulate_window_mutation_observation(
        &self,
        window: AnyWindowHandle,
        domain: WindowMutationDomain,
        facts: WindowPlatformFacts,
    ) -> bool {
        let emitted = self
            .test_window(window)
            .simulate_window_mutation_observation(domain, facts);
        self.run_until_parked();
        emitted
    }

    /// Emits supplied backend facts with an explicit asynchronous terminal result.
    pub fn simulate_window_mutation_terminal(
        &self,
        window: AnyWindowHandle,
        domain: WindowMutationDomain,
        terminal: PlatformWindowMutationTerminal,
        facts: WindowPlatformFacts,
    ) -> bool {
        let emitted = self
            .test_window(window)
            .simulate_window_mutation_terminal(domain, terminal, facts);
        self.run_until_parked();
        emitted
    }

    /// Emits an explicitly generation-bound terminal mutation result.
    pub fn simulate_window_mutation_terminal_for_generation(
        &self,
        window: AnyWindowHandle,
        domain: WindowMutationDomain,
        generation: u64,
        terminal: PlatformWindowMutationTerminal,
        facts: WindowPlatformFacts,
    ) -> bool {
        let emitted = self
            .test_window(window)
            .simulate_window_mutation_terminal_for_generation(domain, generation, terminal, facts);
        self.run_until_parked();
        emitted
    }

    /// Activates accessibility for a test window and drains the resulting frame work.
    ///
    /// Returns `false` when the application was created with accessibility disabled.
    /// Repeated calls request another full tree, matching AccessKit activation semantics.
    pub fn activate_accessibility(&self, window: AnyWindowHandle) -> bool {
        let activated = self.test_window(window).activate_accessibility();
        self.run_until_parked();
        activated
    }

    /// Deactivates accessibility for a test window and drains the resulting frame work.
    ///
    /// Returns `false` when accessibility was not active for the window.
    pub fn deactivate_accessibility(&self, window: AnyWindowHandle) -> bool {
        let deactivated = self.test_window(window).deactivate_accessibility();
        self.run_until_parked();
        deactivated
    }

    /// Returns the latest final accessibility update delivered by the window to the platform.
    ///
    /// Nodes are sorted by ID for deterministic assertions. Node identity, focus, bounds, and
    /// relationship ordering are preserved exactly as emitted by GPUI.
    pub fn latest_accessibility_tree_update(
        &self,
        window: AnyWindowHandle,
    ) -> Option<accesskit::TreeUpdate> {
        self.test_window(window).latest_accessibility_tree_update()
    }

    /// Returns all final accessibility updates delivered to the platform in arrival order.
    ///
    /// Each update is normalized by node ID without changing semantic relationship ordering.
    /// Unexpected deliveries after deactivation remain visible so tests can detect lifecycle bugs.
    pub fn accessibility_tree_update_history(
        &self,
        window: AnyWindowHandle,
    ) -> Vec<accesskit::TreeUpdate> {
        self.test_window(window).accessibility_tree_update_history()
    }

    /// Dispatches an AccessKit request through the platform callback for a test window.
    ///
    /// Returns `false` when accessibility is inactive or unavailable for the window.
    pub fn dispatch_accessibility_action(
        &self,
        window: AnyWindowHandle,
        request: accesskit::ActionRequest,
    ) -> bool {
        let dispatched = self
            .test_window(window)
            .dispatch_accessibility_action(request);
        self.run_until_parked();
        dispatched
    }

    /// Returns true if there's an alert dialog open.
    pub fn expect_restart(&self) -> oneshot::Receiver<Option<PathBuf>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.test_platform.expect_restart.borrow_mut().replace(tx);
        rx
    }

    /// Simulates the system waking from sleep.
    pub fn simulate_system_wake(&self) {
        self.test_platform.simulate_system_wake();
        self.run_until_parked();
    }

    /// Simulates the platform requesting that the application open URLs.
    pub fn simulate_open_urls(&self, urls: impl IntoIterator<Item = impl Into<String>>) {
        self.test_platform
            .simulate_open_urls(urls.into_iter().map(Into::into).collect());
        self.run_until_parked();
    }

    /// Simulates the platform reopening an already-running application.
    pub fn simulate_reopen(&self) {
        self.test_platform.simulate_reopen();
        self.run_until_parked();
    }

    /// Simulates the platform opening the application menu.
    pub fn simulate_will_open_app_menu(&self) {
        self.test_platform.simulate_will_open_app_menu();
        self.run_until_parked();
    }

    /// Simulates selecting an application menu action.
    pub fn simulate_app_menu_action(&self, action: &dyn Action) {
        self.test_platform.simulate_app_menu_action(action);
        self.run_until_parked();
    }

    /// Simulates the platform synchronously validating an application menu action.
    pub fn simulate_validate_app_menu_command(&self, action: &dyn Action) -> bool {
        self.test_platform
            .simulate_validate_app_menu_command(action)
    }

    /// Causes the given sources to be returned if the application queries for screen
    /// capture sources.
    pub fn set_screen_capture_sources(&self, sources: Vec<TestScreenCaptureSource>) {
        self.test_platform.set_screen_capture_sources(sources);
    }

    /// Returns all windows open in the test.
    pub fn windows(&self) -> Vec<AnyWindowHandle> {
        self.app.borrow().windows()
    }

    /// Returns whether the test platform received an application quit request.
    pub fn did_quit(&self) -> bool {
        self.test_platform.did_quit()
    }

    /// Simulates the user closing a platform window.
    ///
    /// Returns true when the request began a logical or native terminal transition.
    ///
    /// This exercises the same App-side removal path that
    /// `QuitMode::LastWindowClosed` observes when the logical window is removed.
    pub fn simulate_window_close(&mut self, window: AnyWindowHandle) -> bool {
        self.simulate_window_close_request(window)
            .terminal_transition_started()
    }

    /// Simulates a user close request and exposes both App and native outcomes.
    ///
    /// Use this when a test must distinguish a native terminal veto from a rejected
    /// user close request. A native veto does not by itself indicate whether a
    /// component-specific close coordinator accepted the request; consult that
    /// coordinator's state separately.
    pub fn simulate_window_close_request(
        &mut self,
        window: AnyWindowHandle,
    ) -> TestWindowCloseRequestOutcome {
        let platform_window = self.test_window(window);
        let native_close_allowed = platform_window.should_close();
        let mut logical_window_removed = !self.windows().contains(&window);
        if !native_close_allowed && !logical_window_removed {
            return TestWindowCloseRequestOutcome {
                native_close_allowed,
                logical_window_removed,
                native_terminal_started: false,
            };
        }

        if !logical_window_removed && native_close_allowed {
            let _ = platform_window.simulate_close();
        }
        self.background_executor.run_until_parked();
        logical_window_removed = !self.windows().contains(&window);
        let native_terminal_started = platform_window.is_native_terminal();
        TestWindowCloseRequestOutcome {
            native_close_allowed,
            logical_window_removed,
            native_terminal_started,
        }
    }

    /// Run the given task on the main thread.
    #[track_caller]
    pub fn spawn<Fut, R>(&self, f: impl FnOnce(AsyncApp) -> Fut) -> Task<R>
    where
        Fut: Future<Output = R> + 'static,
        R: 'static,
    {
        self.foreground_executor.spawn(f(self.to_async()))
    }

    /// true if the given global is defined
    pub fn has_global<G: Global>(&self) -> bool {
        let app = self.app.borrow();
        app.has_global::<G>()
    }

    /// runs the given closure with a reference to the global
    /// panics if `has_global` would return false.
    pub fn read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> R {
        let app = self.app.borrow();
        read(app.global(), &app)
    }

    /// runs the given closure with a reference to the global (if set)
    pub fn try_read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> Option<R> {
        let lock = self.app.borrow();
        Some(read(lock.try_global()?, &lock))
    }

    /// sets the global in this context.
    pub fn set_global<G: Global>(&mut self, global: G) {
        let mut lock = self.app.borrow_mut();
        lock.update(|cx| cx.set_global(global))
    }

    /// updates the global in this context. (panics if `has_global` would return false)
    pub fn update_global<G: Global, R>(&mut self, update: impl FnOnce(&mut G, &mut App) -> R) -> R {
        let mut lock = self.app.borrow_mut();
        lock.update(|cx| cx.update_global(update))
    }

    /// Returns an `AsyncApp` which can be used to run tasks that expect to be on a background
    /// thread on the current thread in tests.
    pub fn to_async(&self) -> AsyncApp {
        AsyncApp {
            app: Rc::downgrade(&self.app),
            background_executor: self.background_executor.clone(),
            foreground_executor: self.foreground_executor.clone(),
        }
    }

    /// Wait until there are no more pending tasks.
    pub fn run_until_parked(&self) {
        self.dispatcher.run_until_parked();
    }

    /// Overrides the platform-reported mouse button state for tests.
    ///
    /// Pass `None` to return the test platform to its default unsupported state.
    pub fn set_platform_mouse_button_is_pressed(&self, button: MouseButton, pressed: Option<bool>) {
        self.test_platform
            .set_mouse_button_is_pressed(button, pressed);
    }

    /// Overrides the platform-reported window under the mouse cursor for tests.
    pub fn set_platform_hovered_window(&self, window: Option<AnyWindowHandle>) {
        let test_window = window.and_then(|window| {
            self.app
                .borrow_mut()
                .update_window(window, |_, window, _| {
                    window.platform_window.as_test().cloned()
                })
                .ok()
                .flatten()
        });
        self.test_platform.set_hovered_window(test_window);
    }

    /// Returns the cursor style most recently requested by the test platform.
    pub fn platform_cursor_style(&self) -> CursorStyle {
        self.test_platform.cursor_style()
    }

    /// Overrides the platform-reported window stack for tests.
    pub fn set_platform_window_stack(&self, windows: Option<Vec<AnyWindowHandle>>) {
        let test_windows = windows.map(|windows| {
            windows
                .into_iter()
                .filter_map(|window| {
                    self.app
                        .borrow_mut()
                        .update_window(window, |_, window, _| {
                            window.platform_window.as_test().cloned()
                        })
                        .ok()
                        .flatten()
                })
                .collect()
        });
        self.test_platform.set_window_stack(test_windows);
    }

    /// Overrides the complete native hit stack for one physical desktop point.
    ///
    /// Queries at every other point remain unavailable so tests cannot accidentally reuse a
    /// point-scoped observation.
    pub fn set_platform_window_hit_stack(&self, stack: PlatformWindowHitStack) {
        self.test_platform.set_window_hit_stack(stack);
    }

    /// Overrides one test window's physical client geometry and target scale.
    pub fn set_platform_window_physical_client_geometry(
        &self,
        window: AnyWindowHandle,
        bounds: Option<Bounds<crate::DevicePixels>>,
        scale_factor: f32,
    ) {
        self.test_window(window)
            .set_physical_client_geometry(bounds, scale_factor);
    }

    /// Overrides the native drag hysteresis sampled by subsequently started test drags.
    ///
    /// Pass `None` to model a platform that cannot report this native fact.
    pub fn set_platform_native_drag_hysteresis(
        &self,
        hysteresis: Option<crate::PlatformNativeDragHysteresis>,
    ) {
        self.test_platform.set_native_drag_hysteresis(hysteresis);
    }

    /// Overrides one test window's native pointer-capture release result.
    pub fn set_pointer_capture_release_callback(
        &self,
        window: AnyWindowHandle,
        callback: impl FnMut(u64) -> PlatformPointerCaptureReleaseOutcome + 'static,
    ) {
        let mut callback = callback;
        self.test_window(window)
            .set_pointer_capture_release_callback(move |generation, _| callback(generation));
    }

    /// Overrides whether the test platform can report the focused window.
    pub fn set_platform_focused_window_available(&self, available: bool) {
        self.test_platform.set_focused_window_available(available);
    }

    /// Overrides whether the test platform can report the hovered window.
    pub fn set_platform_hovered_window_available(&self, available: bool) {
        self.test_platform.set_hovered_window_available(available);
    }

    /// Overrides whether the test platform advertises live pointer-input mutation.
    pub fn set_platform_pointer_input_mutation_supported(&self, supported: bool) {
        self.test_platform
            .set_pointer_input_mutation_supported(supported);
    }

    /// Overrides the complete test-platform window mutation capability matrix.
    pub fn set_platform_window_mutation_capabilities(
        &self,
        capabilities: PlatformWindowMutationCapabilities,
    ) {
        self.test_platform
            .set_window_mutation_capabilities(capabilities);
    }

    /// Overrides the complete test-platform window creation capability matrix.
    pub fn set_platform_window_creation_capabilities(
        &self,
        capabilities: PlatformWindowCreationCapabilities,
    ) {
        self.test_platform
            .set_window_creation_capabilities(capabilities);
    }

    /// Overrides whether the test platform can open independent platform viewport windows.
    pub fn set_platform_viewport_windows(&self, supported: bool) {
        self.test_platform.set_platform_viewport_windows(supported);
    }

    /// Makes the next TestPlatform window fail during `PlatformWindow::map_window`.
    pub fn fail_next_window_map(&self, message: impl Into<String>) {
        self.test_platform.fail_next_window_map(message);
    }

    /// Makes the next TestPlatform window synchronously close during `PlatformWindow::map_window`.
    pub fn close_next_window_during_map(&self) {
        self.test_platform.close_next_window_during_map();
    }

    /// Overrides the immutable creation visibility fact reported by the next test window.
    pub fn set_next_window_creation_show_fact(&self, show: bool) {
        self.test_platform.set_next_window_creation_show_fact(show);
    }

    /// Makes the next TestPlatform window reject both initial-presentation command attempts.
    pub fn reject_next_window_initial_presentation(&self) {
        self.test_platform.reject_next_window_initial_presentation();
    }

    /// Makes the next TestPlatform window synchronously close during its hidden first present.
    pub fn close_next_window_during_initial_presentation(&self) {
        self.test_platform
            .close_next_window_during_initial_presentation();
    }

    /// Holds frame requests made by the next TestPlatform window until explicitly released.
    pub fn defer_next_window_frame_requests(&self) {
        self.test_platform.defer_next_window_frame_requests();
    }

    /// Delivers one deferred frame request for `window` while keeping later requests deferred.
    pub fn step_deferred_window_frame_request(&self, window: AnyWindowHandle) -> bool {
        let stepped = self
            .test_window(window)
            .step_deferred_frame_request_for_test();
        self.run_until_parked();
        stepped
    }

    /// Holds the native terminal callback for `window` after its logical GPUI removal.
    ///
    /// This models backends whose owning platform window closes asynchronously. The returned
    /// hold must remain alive until the test is ready to deliver the native `Closed` event.
    pub fn hold_window_native_terminal(
        &self,
        window: AnyWindowHandle,
    ) -> TestWindowNativeTerminalHold {
        let platform_window = self.test_window(window);
        assert!(
            platform_window.defer_native_terminal(),
            "native terminal can only be held for a live TestPlatform window"
        );
        TestWindowNativeTerminalHold {
            window: Some(platform_window),
        }
    }

    /// Controls whether `window` can acknowledge presentation shutdown.
    pub fn block_window_presentation_shutdown(&self, window: AnyWindowHandle, blocked: bool) {
        self.test_window(window)
            .block_presentation_shutdown(blocked);
    }

    /// Retains a read-only activation counter for `window` across logical window removal.
    pub fn window_activation_probe(&self, window: AnyWindowHandle) -> TestWindowActivationProbe {
        TestWindowActivationProbe {
            window: self.test_window(window),
        }
    }

    /// Simulate dispatching an action to the currently focused node in the window.
    pub fn dispatch_action<A>(&mut self, window: AnyWindowHandle, action: A)
    where
        A: Action,
    {
        window
            .update(self, |_, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx)
            })
            .unwrap();

        self.background_executor.run_until_parked()
    }

    /// simulate_keystrokes takes a space-separated list of keys to type.
    /// cx.simulate_keystrokes("cmd-shift-p b k s p enter")
    /// in an editor app, this can run backspace on the current editor through the command palette.
    /// This will also run the background executor until it's parked.
    pub fn simulate_keystrokes(&mut self, window: AnyWindowHandle, keystrokes: &str) {
        for keystroke in keystrokes
            .split(' ')
            .map(Keystroke::parse)
            .map(Result::unwrap)
        {
            self.dispatch_keystroke(window, keystroke);
        }

        self.background_executor.run_until_parked()
    }

    /// simulate_input takes a string of text to type.
    /// cx.simulate_input("abc")
    /// will type abc into your current editor
    /// This will also run the background executor until it's parked.
    pub fn simulate_input(&mut self, window: AnyWindowHandle, input: &str) {
        for keystroke in input.split("").map(Keystroke::parse).map(Result::unwrap) {
            self.dispatch_keystroke(window, keystroke);
        }

        self.background_executor.run_until_parked()
    }

    /// Simulates an IME composition update through the focused platform input handler.
    pub fn simulate_marked_text(
        &mut self,
        window: AnyWindowHandle,
        replacement_range: Option<Range<usize>>,
        text: &str,
        selected_range: Option<Range<usize>>,
    ) {
        let input_handler_slot = self
            .update_window(window, |_, window, _| {
                window.platform_window.input_handler_slot_for_test()
            })
            .expect("test window should be available")
            .expect("focused element should install a platform input handler");
        input_handler_slot
            .with_handler(|input_handler| {
                input_handler.replace_and_mark_text_in_range(
                    replacement_range,
                    text,
                    selected_range,
                )
            })
            .expect("focused element should retain a live platform input handler");
        self.background_executor.run_until_parked();
    }

    /// dispatches a single Keystroke (see also `simulate_keystrokes` and `simulate_input`)
    pub fn dispatch_keystroke(&mut self, window: AnyWindowHandle, keystroke: Keystroke) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_keystroke(keystroke, cx)
        })
        .unwrap();
    }

    /// Simulates an input event on the given window and returns the resulting dispatch facts.
    ///
    /// This is the stable test-harness path for asserting default input consumption and
    /// propagation after simulated mouse, wheel, or keyboard input.
    pub fn simulate_event_result<E: InputEvent>(
        &mut self,
        window: AnyWindowHandle,
        event: E,
    ) -> DispatchEventResult {
        let result = self
            .update_window(window, |_, window, cx| {
                window.dispatch_event(event.to_platform_input(), cx)
            })
            .unwrap();
        self.record_input_dispatch(result);
        self.background_executor.run_until_parked();
        result
    }

    /// Simulates an input event on the given window.
    pub fn simulate_event<E: InputEvent>(&mut self, window: AnyWindowHandle, event: E) {
        self.simulate_event_result(window, event);
    }

    /// Simulates an input event and returns a stable summary of dispatch side effects.
    pub fn simulate_event_with_dispatch_snapshot<E: InputEvent>(
        &mut self,
        window: AnyWindowHandle,
        event: E,
    ) -> TestInputDispatchSnapshot {
        TestInputDispatchSnapshot::from(self.simulate_event_result(window, event))
    }

    /// Return the most recent dispatch result recorded by the window.
    pub fn last_dispatch_event_result(
        &mut self,
        window: AnyWindowHandle,
    ) -> Option<DispatchEventResult> {
        self.update_window(window, |_, window, _| window.last_dispatch_event_result())
            .unwrap()
    }

    /// Returns whether the element with the given debug selector owns focus in the window.
    pub fn debug_selector_is_focused_in_window(
        &mut self,
        window: AnyWindowHandle,
        selector: &str,
    ) -> bool {
        self.update_window(window, |_, window, _| {
            let Some(focus_id) = window.rendered_frame.debug_focus_handles.get(selector) else {
                return false;
            };
            window.focus == Some(*focus_id)
        })
        .unwrap()
    }

    /// Returns the first debug selector associated with the focused element in the window.
    pub fn focused_debug_selector_in_window(&mut self, window: AnyWindowHandle) -> Option<String> {
        self.update_window(window, |_, window, _| {
            let focused = window.focus?;
            window
                .rendered_frame
                .debug_focus_handles
                .iter()
                .filter_map(|(selector, focus_id)| (*focus_id == focused).then(|| selector.clone()))
                .min()
        })
        .unwrap()
    }

    /// Returns the most recently created test platform window, including one whose
    /// synchronous creation transaction later failed before registry commit.
    #[cfg(test)]
    pub(crate) fn last_created_test_window(&self) -> Option<TestWindow> {
        self.test_platform.last_created_window()
    }

    /// Returns the `TestWindow` backing the given handle.
    pub(crate) fn test_window(&self, window: AnyWindowHandle) -> TestWindow {
        self.app
            .borrow_mut()
            .windows
            .get_mut(window.id)
            .unwrap()
            .as_deref_mut()
            .unwrap()
            .platform_window
            .as_test()
            .unwrap()
            .clone()
    }

    /// Returns a stream of notifications whenever the Entity is updated.
    pub fn notifications<T: 'static>(
        &mut self,
        entity: &Entity<T>,
    ) -> impl Stream<Item = ()> + use<T> {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        self.update(|cx| {
            cx.observe(entity, {
                let tx = tx.clone();
                move |_, _| {
                    let _ = tx.unbounded_send(());
                }
            })
            .detach();
            cx.observe_release(entity, move |_, _| tx.close_channel())
                .detach()
        });
        rx
    }

    /// Returns a stream of events emitted by the given Entity.
    pub fn events<Evt, T: 'static + EventEmitter<Evt>>(
        &mut self,
        entity: &Entity<T>,
    ) -> futures::channel::mpsc::UnboundedReceiver<Evt>
    where
        Evt: 'static + Clone,
    {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        entity
            .update(self, |_, cx: &mut Context<T>| {
                cx.subscribe(entity, move |_entity, _handle, event, _cx| {
                    let _ = tx.unbounded_send(event.clone());
                })
            })
            .detach();
        rx
    }

    /// Runs until the given condition becomes true. (Prefer `run_until_parked` if you
    /// don't need to jump in at a specific time).
    pub async fn condition<T: 'static>(
        &mut self,
        entity: &Entity<T>,
        mut predicate: impl FnMut(&mut T, &mut Context<T>) -> bool,
    ) {
        let timer = self.executor().timer(Duration::from_secs(3));
        let mut notifications = self.notifications(entity);

        use futures::FutureExt as _;
        use futures_concurrency::future::Race as _;

        (
            async {
                loop {
                    if entity.update(self, &mut predicate) {
                        return Ok(());
                    }

                    if notifications.next().await.is_none() {
                        bail!("entity dropped")
                    }
                }
            },
            timer.map(|_| Err(anyhow!("condition timed out"))),
        )
            .race()
            .await
            .unwrap();
    }

    /// Set a name for this App.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_name(&mut self, name: &'static str) {
        self.update(|cx| cx.name = Some(name))
    }
}

impl<T: 'static> Entity<T> {
    /// Block until the next event is emitted by the entity, then return it.
    pub fn next_event<Event>(&self, cx: &mut TestAppContext) -> impl Future<Output = Event>
    where
        Event: Send + Clone + 'static,
        T: EventEmitter<Event>,
    {
        let (tx, mut rx) = oneshot::channel();
        let mut tx = Some(tx);
        let subscription = self.update(cx, |_, cx| {
            cx.subscribe(self, move |_, _, event, _| {
                if let Some(tx) = tx.take() {
                    _ = tx.send(event.clone());
                }
            })
        });

        async move {
            let event = rx.await.expect("no event emitted");
            drop(subscription);
            event
        }
    }
}

impl<V: 'static> Entity<V> {
    /// Returns a future that resolves when the view is next updated.
    pub fn next_notification(
        &self,
        advance_clock_by: Duration,
        cx: &TestAppContext,
    ) -> impl Future<Output = ()> {
        use postage::prelude::{Sink as _, Stream as _};

        let (mut tx, mut rx) = postage::mpsc::channel(1);
        let subscription = cx.app.borrow_mut().observe(self, move |_, _| {
            tx.try_send(()).ok();
        });

        cx.executor().advance_clock(advance_clock_by);

        async move {
            rx.recv()
                .await
                .expect("entity dropped while test was waiting for its next notification");
            drop(subscription);
        }
    }
}

impl<V> Entity<V> {
    /// Returns a future that resolves when the condition becomes true.
    pub fn condition<Evt>(
        &self,
        cx: &TestAppContext,
        mut predicate: impl FnMut(&V, &App) -> bool,
    ) -> impl Future<Output = ()>
    where
        Evt: 'static,
        V: EventEmitter<Evt>,
    {
        use postage::prelude::{Sink as _, Stream as _};

        let (tx, mut rx) = postage::mpsc::channel(1024);

        let mut cx = cx.app.borrow_mut();
        let subscriptions = (
            cx.observe(self, {
                let mut tx = tx.clone();
                move |_, _| {
                    tx.blocking_send(()).ok();
                }
            }),
            cx.subscribe(self, {
                let mut tx = tx;
                move |_, _: &Evt, _| {
                    tx.blocking_send(()).ok();
                }
            }),
        );

        let cx = cx.this.upgrade().unwrap();
        let handle = self.downgrade();

        async move {
            loop {
                {
                    let cx = cx.borrow();
                    let cx = &*cx;
                    if predicate(
                        handle
                            .upgrade()
                            .expect("view dropped with pending condition")
                            .read(cx),
                        cx,
                    ) {
                        break;
                    }
                }

                rx.recv()
                    .await
                    .expect("view dropped with pending condition");
            }
            drop(subscriptions);
        }
    }
}

use derive_more::{Deref, DerefMut};

use super::{Context, Entity};
#[derive(Deref, DerefMut, Clone)]
/// A VisualTestContext is the test-equivalent of a `Window` and `App`. It allows you to
/// run window-specific test code. It can be dereferenced to a `TextAppContext`.
pub struct VisualTestContext {
    #[deref]
    #[deref_mut]
    /// cx is the original TestAppContext (you can more easily access this using Deref)
    pub cx: TestAppContext,
    window: AnyWindowHandle,
}

impl VisualTestContext {
    /// Provides a `Window` and `App` for the duration of the closure.
    pub fn update<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        self.cx
            .update_window(self.window, |_, window, cx| f(window, cx))
            .unwrap()
    }

    /// Creates a new VisualTestContext. You would typically shadow the passed in
    /// TestAppContext with this, as this is typically more useful.
    /// `let cx = VisualTestContext::from_window(window, cx);`
    pub fn from_window(window: AnyWindowHandle, cx: &TestAppContext) -> Self {
        Self {
            cx: cx.clone(),
            window,
        }
    }

    /// Wait until there are no more pending tasks.
    pub fn run_until_parked(&self) {
        self.cx.background_executor.run_until_parked();
    }

    /// Simulates the platform delivering a frame request.
    pub fn simulate_frame(&mut self, options: RequestFrameOptions) -> bool {
        let handled = self.test_window(self.window).simulate_frame(options);
        self.background_executor.run_until_parked();
        handled
    }

    /// Activates accessibility for this test window.
    pub fn activate_accessibility(&self) -> bool {
        self.cx.activate_accessibility(self.window)
    }

    /// Deactivates accessibility for this test window.
    pub fn deactivate_accessibility(&self) -> bool {
        self.cx.deactivate_accessibility(self.window)
    }

    /// Returns the latest normalized final accessibility update for this test window.
    pub fn latest_accessibility_tree_update(&self) -> Option<accesskit::TreeUpdate> {
        self.cx.latest_accessibility_tree_update(self.window)
    }

    /// Returns the normalized final accessibility update history for this test window.
    pub fn accessibility_tree_update_history(&self) -> Vec<accesskit::TreeUpdate> {
        self.cx.accessibility_tree_update_history(self.window)
    }

    /// Dispatches an AccessKit request through this test window's platform callback.
    pub fn dispatch_accessibility_action(&self, request: accesskit::ActionRequest) -> bool {
        self.cx.dispatch_accessibility_action(self.window, request)
    }

    /// Dispatch the action to the currently focused node.
    pub fn dispatch_action<A>(&mut self, action: A)
    where
        A: Action,
    {
        self.cx.dispatch_action(self.window, action)
    }

    /// Read the title off the window (set by `Window#set_window_title`)
    pub fn window_title(&mut self) -> Option<String> {
        self.cx.test_window(self.window).0.lock().title.clone()
    }

    /// Read the document path off the window (set by `Window#set_document_path`)
    pub fn document_path(&mut self) -> Option<std::path::PathBuf> {
        self.cx
            .test_window(self.window)
            .0
            .lock()
            .document_path
            .clone()
    }

    /// Simulate a sequence of keystrokes `cx.simulate_keystrokes("cmd-p escape")`
    /// Automatically runs until parked.
    pub fn simulate_keystrokes(&mut self, keystrokes: &str) {
        self.cx.simulate_keystrokes(self.window, keystrokes)
    }

    /// Simulate typing text `cx.simulate_input("hello")`
    /// Automatically runs until parked.
    pub fn simulate_input(&mut self, input: &str) {
        self.cx.simulate_input(self.window, input)
    }

    /// Simulates an IME composition update through the focused platform input handler.
    pub fn simulate_marked_text(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        selected_range: Option<Range<usize>>,
    ) {
        self.cx
            .simulate_marked_text(self.window, replacement_range, text, selected_range)
    }

    /// Simulate a mouse move event to the given point
    pub fn simulate_mouse_move(
        &mut self,
        position: Point<Pixels>,
        button: impl Into<Option<MouseButton>>,
        modifiers: Modifiers,
    ) {
        self.simulate_event(MouseMoveEvent {
            position,
            modifiers,
            pressed_button: button.into(),
        })
    }

    /// Simulate a mouse down event to the given point
    pub fn simulate_mouse_down(
        &mut self,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(MouseDownEvent {
            position,
            modifiers,
            button,
            click_count: 1,
            first_mouse: false,
        })
    }

    /// Simulate a mouse up event to the given point
    pub fn simulate_mouse_up(
        &mut self,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(MouseUpEvent {
            position,
            modifiers,
            button,
            click_count: 1,
        })
    }

    /// Simulate a mouse exit event at the given point.
    pub fn simulate_mouse_exit(
        &mut self,
        position: Point<Pixels>,
        button: impl Into<Option<MouseButton>>,
        modifiers: Modifiers,
    ) {
        self.simulate_event(MouseExitEvent {
            position,
            modifiers,
            pressed_button: button.into(),
        })
    }

    /// Simulate a mouse drag from one point to another on the current window.
    pub fn simulate_drag(
        &mut self,
        start: Point<Pixels>,
        end: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_mouse_down(start, button, modifiers);
        self.simulate_mouse_move(end, Some(button), modifiers);
        self.simulate_mouse_up(end, button, modifiers);
    }

    /// Simulate a primary mouse click at the given point
    pub fn simulate_click(&mut self, position: Point<Pixels>, modifiers: Modifiers) {
        self.simulate_event(MouseDownEvent {
            position,
            modifiers,
            button: MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        });
        self.simulate_event(MouseUpEvent {
            position,
            modifiers,
            button: MouseButton::Left,
            click_count: 1,
        });
    }

    /// Simulate a modifiers changed event
    pub fn simulate_modifiers_change(&mut self, modifiers: Modifiers) {
        self.simulate_event(ModifiersChangedEvent {
            modifiers,
            capslock: Capslock { on: false },
        })
    }

    /// Simulate a capslock changed event
    pub fn simulate_capslock_change(&mut self, on: bool) {
        self.simulate_event(ModifiersChangedEvent {
            modifiers: Modifiers::none(),
            capslock: Capslock { on },
        })
    }

    /// Simulates the user resizing the window to the new size.
    pub fn simulate_resize(&self, size: Size<Pixels>) {
        self.simulate_window_resize(self.window, size)
    }

    /// debug_bounds returns the bounds of the element with the given selector.
    pub fn debug_bounds(&mut self, selector: &str) -> Option<Bounds<Pixels>> {
        self.update(|window, _| window.rendered_frame.debug_bounds.get(selector).copied())
    }

    /// Returns rendered debug selectors that begin with the given prefix.
    pub fn debug_selectors_with_prefix(&mut self, prefix: &str) -> Vec<String> {
        self.update(|window, _| {
            let mut selectors = window
                .rendered_frame
                .debug_bounds
                .keys()
                .filter(|selector| selector.starts_with(prefix))
                .cloned()
                .collect::<Vec<_>>();
            selectors.sort_unstable();
            selectors
        })
    }

    /// Returns whether the element with the given debug selector owns the current focus.
    pub fn debug_selector_is_focused(&mut self, selector: &str) -> bool {
        self.update(|window, _| {
            let Some(focus_id) = window.rendered_frame.debug_focus_handles.get(selector) else {
                return false;
            };
            window.focus == Some(*focus_id)
        })
    }

    /// Returns the first debug selector associated with the currently focused element.
    pub fn focused_debug_selector(&mut self) -> Option<String> {
        self.update(|window, _| {
            let focused = window.focus?;
            window
                .rendered_frame
                .debug_focus_handles
                .iter()
                .filter_map(|(selector, focus_id)| (*focus_id == focused).then(|| selector.clone()))
                .min()
        })
    }

    /// Draw an element to the window. Useful for simulating events or actions
    pub fn draw<E>(
        &mut self,
        origin: Point<Pixels>,
        space: impl Into<Size<AvailableSpace>>,
        f: impl FnOnce(&mut Window, &mut App) -> E,
    ) -> (E::RequestLayoutState, E::PrepaintState)
    where
        E: Element,
    {
        self.update(|window, cx| {
            let _arena_scope = ElementArenaScope::enter(&cx.element_arena);

            window.invalidator.set_phase(DrawPhase::Prepaint);
            let mut element = Drawable::new(f(window, cx));
            element.layout_as_root(space.into(), window, cx);
            window.with_absolute_element_offset(origin, |window| element.prepaint(window, cx));

            window.invalidator.set_phase(DrawPhase::Paint);
            let (request_layout_state, prepaint_state) = element.paint(window, cx);

            window.invalidator.set_phase(DrawPhase::None);
            window.accept_visual_test_frame_transfers();
            window.refresh();

            drop(element);
            cx.element_arena.borrow_mut().clear();

            (request_layout_state, prepaint_state)
        })
    }

    /// Simulate an event from the platform, e.g. a ScrollWheelEvent
    /// Make sure you've called [VisualTestContext::draw] first!
    pub fn simulate_event<E: InputEvent>(&mut self, event: E) {
        self.simulate_event_with_dispatch_snapshot(event);
    }

    /// Simulate an event and return a stable summary of dispatch side effects.
    ///
    /// This lets tests assert default-input consumption and propagation without
    /// inspecting a private render plan or reading the transient window flag directly.
    pub fn simulate_event_with_dispatch_snapshot<E: InputEvent>(
        &mut self,
        event: E,
    ) -> TestInputDispatchSnapshot {
        let result = self
            .cx
            .test_window(self.window)
            .simulate_input_result(event.to_platform_input());
        let snapshot = self.cx.record_input_dispatch(result);
        self.background_executor.run_until_parked();
        snapshot
    }

    /// Return the most recent dispatch result recorded by this window.
    pub fn last_dispatch_event_result(&mut self) -> Option<DispatchEventResult> {
        self.cx.last_dispatch_event_result(self.window)
    }

    /// Simulates the user blurring the window.
    pub fn deactivate_window(&mut self) {
        if Some(self.window) == self.test_platform.active_window() {
            self.test_platform.set_active_window(None)
        }
        self.background_executor.run_until_parked();
    }

    /// Simulates the user closing the window.
    /// Returns true if the window was closed.
    pub fn simulate_close(&mut self) -> bool {
        let platform_window = self.cx.test_window(self.window);
        let native_close_allowed = platform_window.should_close();
        let app_close_committed = !self.cx.windows().contains(&self.window);
        if !native_close_allowed && !app_close_committed {
            return false;
        }

        let closed = app_close_committed || platform_window.simulate_close();
        self.background_executor.run_until_parked();
        closed
    }

    /// Get an &mut VisualTestContext (which is mostly what you need to pass to other methods).
    /// This method internally retains the VisualTestContext until the end of the test.
    pub fn into_mut(self) -> &'static mut Self {
        let ptr = Box::into_raw(Box::new(self));
        // safety: on_quit will be called after the test has finished.
        // the executor will ensure that all tasks related to the test have stopped.
        // so there is no way for cx to be accessed after on_quit is called.
        // todo: This is unsound under stacked borrows (also tree borrows probably?)
        // the mutable reference invalidates `ptr` which is later used in the closure
        let cx = unsafe { &mut *ptr };
        cx.on_quit(move || unsafe {
            drop(Box::from_raw(ptr));
        });
        cx
    }
}

impl AppContext for VisualTestContext {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, _, cx| cx.new(build_entity))
            .expect("window was unexpectedly closed")
    }

    fn reserve_entity<T: 'static>(&mut self) -> crate::Reservation<T> {
        self.cx.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: crate::Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, _, cx| {
                cx.insert_entity(reservation, build_entity)
            })
            .expect("window was unexpectedly closed")
    }

    fn update_entity<T, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R
    where
        T: 'static,
    {
        self.cx.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> super::GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        self.cx.as_mut(handle)
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        self.cx.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.cx.update_window(window, f)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        self.cx.with_window(entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        self.cx.read_window(window, read)
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.cx.background_spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        self.cx.read_global(callback)
    }
}

impl VisualContext for VisualTestContext {
    type Result<T> = T;

    /// Get the underlying window handle underlying this context.
    fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Entity<T> {
        self.window
            .update(&mut self.cx, |_, window, cx| {
                cx.new(|cx| build_entity(window, cx))
            })
            .expect("window was unexpectedly closed")
    }

    fn update_window_entity<V: 'static, R>(
        &mut self,
        view: &Entity<V>,
        update: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> R,
    ) -> R {
        let view = view.clone();
        self.cx
            .app
            .borrow_mut()
            .with_window(view.entity_id(), |window, app| {
                view.update(app, |v, cx| update(v, window, cx))
            })
            .expect("entity has no current window; use `update` instead of `update_in`")
    }

    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Entity<V>
    where
        V: 'static + Render,
    {
        self.window
            .update(&mut self.cx, |_, window, cx| {
                window.replace_root(cx, build_view)
            })
            .expect("window was unexpectedly closed")
    }

    fn focus<V: crate::Focusable>(&mut self, view: &Entity<V>) {
        self.window
            .update(&mut self.cx, |_, window, cx| {
                view.read(cx).focus_handle(cx).focus(window, cx)
            })
            .expect("window was unexpectedly closed")
    }
}

impl AnyWindowHandle {
    /// Creates the given view in this window.
    pub fn build_entity<V: Render + 'static>(
        &self,
        cx: &mut TestAppContext,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Entity<V> {
        self.update(cx, |_, window, cx| cx.new(|cx| build_view(window, cx)))
            .unwrap()
    }
}

#[cfg(test)]
mod accessibility_tests;

#[cfg(test)]
mod pointer_session_tests;

#[cfg(test)]
mod portal_anchor_tests;

#[cfg(test)]
mod bring_into_view_tests;

#[cfg(test)]
mod presentation_tests;

#[cfg(test)]
mod transform_tests;

#[cfg(test)]
mod clip_tests;

#[cfg(test)]
mod tests {
    use crate::{
        AnyDrag, AnyView, AppContext as _, Bounds, Context, CursorStyle, DevicePixels, Empty,
        Entity, FocusHandle, InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, Keystroke,
        Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
        PathPromptOptions, Platform, PlatformHoveredWindow, PlatformInput, PlatformWindowHit,
        PlatformWindowHitStack, PlatformWindowPhysicalCoverage, PlatformWindowPhysicalGeometry,
        PointerCaptureHandle, QuitMode, Render, ScrollDelta, ScrollWheelEvent,
        StatefulInteractiveElement, StyleRefinement, Styled, Subscription, TestAppContext,
        TestInputDispatchSnapshot, TouchPhase, VisualContext, VisualTestContext, Window,
        WindowMouseEvent, canvas, deferred, div, point, px, size,
    };
    use std::cell::{Cell, RefCell};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::Arc;

    crate::actions!(window_interceptor_tests, [PendingChordAction]);

    struct FocusDebugView {
        first: FocusHandle,
        second: FocusHandle,
    }

    struct CursorProbeView {
        pointer: bool,
    }

    struct WindowCursorProbeView {
        style: CursorStyle,
    }

    impl Render for CursorProbeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let element = div().size_full();
            if self.pointer {
                element.cursor_pointer().into_any_element()
            } else {
                element.into_any_element()
            }
        }
    }

    impl Render for WindowCursorProbeView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let style = self.style;
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    window.set_window_cursor_style(style);
                },
            )
            .size_full()
        }
    }

    impl Render for FocusDebugView {
        fn render(&mut self, _: &mut crate::Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(
                    div()
                        .id("first")
                        .debug_selector(|| "focus-debug:first".into())
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.first)
                        .child("First"),
                )
                .child(
                    div()
                        .id("second")
                        .debug_selector(|| "focus-debug:second".into())
                        .focusable()
                        .tab_stop(true)
                        .track_focus(&self.second)
                        .child("Second"),
                )
        }
    }

    struct InputDispatchProbe;

    #[derive(Default)]
    struct WindowLocalProbe;

    struct WindowMouseInterceptorProbe {
        consume: Rc<Cell<bool>>,
        events: Rc<RefCell<Vec<&'static str>>>,
        focus: FocusHandle,
        pointer_capture: PointerCaptureHandle,
        mouse_subscription: Option<Subscription>,
        key_subscription: Option<Subscription>,
    }

    struct PrepaintCommitRoot {
        child: Entity<PrepaintCommitProbe>,
    }

    struct PrepaintCommitProbe {
        renders: Rc<Cell<usize>>,
        prepaints: Rc<Cell<usize>>,
        commits: Rc<RefCell<Vec<u64>>>,
        discarded_commits: Rc<Cell<usize>>,
        discarded_resources: Rc<Cell<usize>>,
    }

    struct PrepaintRefreshRoot {
        child: Entity<PrepaintRefreshProbe>,
    }

    struct PrepaintRefreshProbe {
        renders: Rc<Cell<usize>>,
    }

    struct PrepaintResourceDropProbe(Rc<Cell<usize>>);

    impl Drop for PrepaintResourceDropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    impl Render for PrepaintCommitRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(deferred(
                AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full()),
            ))
        }
    }

    impl Render for PrepaintCommitProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            let prepaints = self.prepaints.clone();
            let commits = self.commits.clone();
            let discarded_commits = self.discarded_commits.clone();
            let discarded_resources = self.discarded_resources.clone();
            canvas(
                move |_, window, _| {
                    prepaints.set(prepaints.get() + 1);
                    let discarded_commits = discarded_commits.clone();
                    let discarded_resources_for_drop = discarded_resources.clone();
                    let rejected: std::result::Result<(), ()> = window.transact(|window| {
                        window.record_prepaint_commit(move |_, _| {
                            discarded_commits.set(discarded_commits.get() + 1);
                        });
                        window.next_frame.retained_resources.push(Rc::new(
                            PrepaintResourceDropProbe(discarded_resources_for_drop),
                        ));
                        Err(())
                    });
                    debug_assert!(rejected.is_err());
                    let commits = commits.clone();
                    window.record_prepaint_commit(move |revision, _| {
                        commits.borrow_mut().push(revision);
                    });
                },
                |_, _, _, _| {},
            )
            .size_full()
        }
    }

    impl Render for PrepaintRefreshRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            AnyView::from(self.child.clone()).cached(StyleRefinement::default().size_full())
        }
    }

    impl Render for PrepaintRefreshProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            canvas(
                |_, window, _| {
                    window.record_prepaint_window_commit(|_, window, _| window.refresh());
                },
                |_, _, _, _| {},
            )
            .size_full()
        }
    }

    impl Render for InputDispatchProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("input-dispatch-probe")
                .size_full()
                .capture_scroll_wheel(|_, _, _| {
                    crate::ScrollWheelIntent::handled().stop_propagation()
                })
        }
    }

    impl Render for WindowMouseInterceptorProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.events.clone();
            div()
                .id("window-mouse-interceptor-probe")
                .size_full()
                .focusable()
                .track_focus(&self.focus)
                .track_pointer_capture(&self.pointer_capture)
                .tab_stop(false)
                .capture_key_down({
                    let events = self.events.clone();
                    move |_, _, _| events.borrow_mut().push("node-capture")
                })
                .on_key_down({
                    let events = self.events.clone();
                    move |_, _, _| events.borrow_mut().push("node-bubble")
                })
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    events.borrow_mut().push("node");
                })
                .on_mouse_up(MouseButton::Left, {
                    let events = self.events.clone();
                    move |_, _, _| events.borrow_mut().push("node-up")
                })
                .on_mouse_move({
                    let events = self.events.clone();
                    move |_, _, _| events.borrow_mut().push("node-move")
                })
                .on_scroll_wheel({
                    let events = self.events.clone();
                    move |_, _, _| {
                        events.borrow_mut().push("node-scroll");
                        crate::ScrollWheelIntent::allow_default()
                    }
                })
        }
    }

    #[open_gpui::test]
    fn prepaint_window_commit_can_schedule_a_followup_frame(cx: &mut TestAppContext) {
        let renders = Rc::new(Cell::new(0));
        let (_view, cx) = cx.add_window_view({
            let renders = renders.clone();
            move |_, cx| PrepaintRefreshRoot {
                child: cx.new(|_| PrepaintRefreshProbe { renders }),
            }
        });

        cx.update(|window, cx| {
            window.draw(cx).clear();
            assert!(
                window.refresh_pending_for_test(),
                "a commit-side refresh must schedule the frame that publishes its state"
            );
        });
        let renders_before_followup = renders.get();
        cx.update(|window, cx| window.draw(cx).clear());
        assert!(
            renders.get() > renders_before_followup,
            "a commit-side refresh must bypass cached journals on the followup frame"
        );
    }

    #[open_gpui::test]
    fn cached_deferred_prepaint_commits_once_per_frame_and_rolls_back_transactions(
        cx: &mut TestAppContext,
    ) {
        let renders = Rc::new(Cell::new(0));
        let prepaints = Rc::new(Cell::new(0));
        let commits = Rc::new(RefCell::new(Vec::new()));
        let discarded_commits = Rc::new(Cell::new(0));
        let discarded_resources = Rc::new(Cell::new(0));
        let (_view, cx) = cx.add_window_view({
            let renders = renders.clone();
            let prepaints = prepaints.clone();
            let commits = commits.clone();
            let discarded_commits = discarded_commits.clone();
            let discarded_resources = discarded_resources.clone();
            move |_, cx| PrepaintCommitRoot {
                child: cx.new(|_| PrepaintCommitProbe {
                    renders,
                    prepaints,
                    commits,
                    discarded_commits,
                    discarded_resources,
                }),
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        commits.borrow_mut().clear();
        let fresh_renders = renders.get();
        let fresh_prepaints = prepaints.get();

        let second_revision = cx.update(|window, cx| {
            window.draw(cx).clear();
            window.rendered_frame_revision()
        });
        assert_eq!(renders.get(), fresh_renders);
        assert_eq!(prepaints.get(), fresh_prepaints);
        assert_eq!(commits.borrow().as_slice(), &[second_revision]);
        assert_eq!(discarded_commits.get(), 0);
        assert_eq!(discarded_resources.get(), 1);

        let third_revision = cx.update(|window, cx| {
            window.draw(cx).clear();
            window.rendered_frame_revision()
        });
        assert_eq!(renders.get(), fresh_renders);
        assert_eq!(prepaints.get(), fresh_prepaints);
        assert_eq!(
            commits.borrow().as_slice(),
            &[second_revision, third_revision]
        );
        assert_eq!(discarded_commits.get(), 0);
        assert_eq!(
            discarded_resources.get(),
            1,
            "cached prepaint must not replay rejected transaction resources"
        );
    }

    #[open_gpui::test]
    fn window_state_is_unique_per_type_and_isolated_between_windows(cx: &mut TestAppContext) {
        let first = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let second = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();

        let first_state = cx
            .update_window(first, |_, window, cx| {
                window.use_window_state(cx, |_, _| WindowLocalProbe)
            })
            .expect("first window should remain open");
        let same_first_state = cx
            .update_window(first, |_, window, cx| {
                window.use_window_state(cx, |_, _| WindowLocalProbe)
            })
            .expect("first window should remain open");
        let second_state = cx
            .update_window(second, |_, window, cx| {
                window.use_window_state(cx, |_, _| WindowLocalProbe)
            })
            .expect("second window should remain open");

        assert_eq!(first_state.entity_id(), same_first_state.entity_id());
        assert_ne!(first_state.entity_id(), second_state.entity_id());

        let first_state_weak = first_state.downgrade();
        drop(first_state);
        drop(same_first_state);
        assert!(first_state_weak.upgrade().is_some());
        assert!(cx.simulate_window_close(first));
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(first_state_weak.upgrade().is_none());
        assert!(second_state.downgrade().upgrade().is_some());
    }

    #[open_gpui::test]
    fn window_state_can_be_queried_without_initializing_or_mutating(cx: &mut TestAppContext) {
        let window = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();

        let absent = cx
            .update_window(window, |_, window, _| {
                window.window_state::<WindowLocalProbe>()
            })
            .expect("window should remain open");
        assert!(absent.is_none());

        let initialized = cx
            .update_window(window, |_, window, cx| {
                window.use_window_state(cx, |_, _| WindowLocalProbe)
            })
            .expect("window should remain open");
        let queried = cx
            .update_window(window, |_, window, _| {
                window.window_state::<WindowLocalProbe>()
            })
            .expect("window should remain open")
            .expect("initialized state should be queryable");

        assert_eq!(queried.entity_id(), initialized.entity_id());
    }

    #[open_gpui::test]
    fn window_state_can_retry_after_direct_recursive_initialization_panics(
        cx: &mut TestAppContext,
    ) {
        struct DirectState;
        let (_view, cx) = cx.add_window_view(|_, _| Empty);

        let result = cx.update(|window, cx| {
            catch_unwind(AssertUnwindSafe(|| {
                window.use_window_state::<DirectState>(cx, |window, cx| {
                    let _ = window.use_window_state::<DirectState>(cx, |_, _| DirectState);
                    DirectState
                })
            }))
        });
        assert!(result.is_err(), "direct recursion must panic");

        let retry =
            cx.update(|window, cx| window.use_window_state::<DirectState>(cx, |_, _| DirectState));
        let same = cx.update(|window, cx| {
            window.use_window_state::<DirectState>(cx, |_, _| {
                panic!("the successful retry must remain authoritative")
            })
        });
        assert_eq!(retry.entity_id(), same.entity_id());
    }

    #[open_gpui::test]
    fn window_state_can_retry_all_slots_after_indirect_recursion_panics(cx: &mut TestAppContext) {
        struct StateA;
        struct StateB;
        let (_view, cx) = cx.add_window_view(|_, _| Empty);

        let result = cx.update(|window, cx| {
            catch_unwind(AssertUnwindSafe(|| {
                window.use_window_state::<StateA>(cx, |window, cx| {
                    let _ = window.use_window_state::<StateB>(cx, |window, cx| {
                        let _ = window.use_window_state::<StateA>(cx, |_, _| StateA);
                        StateB
                    });
                    StateA
                })
            }))
        });
        assert!(result.is_err(), "indirect recursion must panic");

        let state_a = cx.update(|window, cx| window.use_window_state::<StateA>(cx, |_, _| StateA));
        let state_b = cx.update(|window, cx| window.use_window_state::<StateB>(cx, |_, _| StateB));
        assert_ne!(state_a.entity_id(), state_b.entity_id());
    }

    #[open_gpui::test]
    fn window_mouse_interceptor_precedes_nodes_and_preserves_single_pass_through(
        cx: &mut TestAppContext,
    ) {
        let consume = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, _cx| {
            let consume = view.consume.clone();
            let events = view.events.clone();
            view.mouse_subscription = Some(window.intercept_window_mouse_events(
                move |event, window, cx| {
                    if !matches!(event, WindowMouseEvent::Down(_)) {
                        return;
                    }
                    events.borrow_mut().push("interceptor");
                    if consume.get() {
                        cx.stop_propagation();
                        window.prevent_default();
                    }
                },
            ));
        });

        let pass_through = cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        });
        assert_eq!(&*events.borrow(), &["interceptor", "node"]);
        assert!(pass_through.propagated());

        events.borrow_mut().clear();
        consume.set(true);
        let consumed = cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        });
        assert_eq!(&*events.borrow(), &["interceptor"]);
        assert!(consumed.propagation_stopped());
        assert!(consumed.default_prevented());
    }

    #[open_gpui::test]
    fn window_mouse_interceptor_gates_pointer_events_and_preserves_mouse_up_cleanup(
        cx: &mut TestAppContext,
    ) {
        fn inputs() -> Vec<(&'static str, &'static str, PlatformInput)> {
            let position = point(px(10.0), px(10.0));
            vec![
                (
                    "interceptor-down",
                    "node",
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::none(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                ),
                (
                    "interceptor-up",
                    "node-up",
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::none(),
                        click_count: 1,
                    }),
                ),
                (
                    "interceptor-move",
                    "node-move",
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        pressed_button: None,
                        modifiers: Modifiers::none(),
                    }),
                ),
                (
                    "interceptor-scroll",
                    "node-scroll",
                    PlatformInput::ScrollWheel(ScrollWheelEvent {
                        position,
                        delta: ScrollDelta::Pixels(point(px(0.0), px(1.0))),
                        modifiers: Modifiers::none(),
                        touch_phase: TouchPhase::Moved,
                    }),
                ),
            ]
        }

        let consume = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, _cx| {
            let consume = view.consume.clone();
            let events = view.events.clone();
            view.mouse_subscription = Some(window.intercept_window_mouse_events(
                move |event, window, cx| {
                    let label = match event {
                        WindowMouseEvent::Down(_) => "interceptor-down",
                        WindowMouseEvent::Up(_) => "interceptor-up",
                        WindowMouseEvent::Move(_) => "interceptor-move",
                        WindowMouseEvent::Exit(_) => "interceptor-exit",
                        WindowMouseEvent::Cancel(_) => "interceptor-cancel",
                        WindowMouseEvent::Pressure(_) => "interceptor-pressure",
                        WindowMouseEvent::Scroll(_) => "interceptor-scroll",
                        WindowMouseEvent::Pinch(_) => "interceptor-pinch",
                        WindowMouseEvent::FileDrop(_) => "interceptor-file-drop",
                    };
                    events.borrow_mut().push(label);
                    if consume.get() {
                        cx.stop_propagation();
                        window.prevent_default();
                    }
                },
            ));
        });

        for (interceptor, node, input) in inputs() {
            events.borrow_mut().clear();
            let result = cx.update(|window, cx| window.dispatch_event(input, cx));
            assert_eq!(&*events.borrow(), &[interceptor, node]);
            assert!(result.propagate);
        }

        consume.set(true);
        for (interceptor, _, input) in inputs() {
            events.borrow_mut().clear();
            if matches!(&input, PlatformInput::MouseUp(_)) {
                cx.update(|window, cx| {
                    let drag_view = cx.new(|_| Empty).into();
                    cx.active_drag = Some(AnyDrag {
                        window_id: window.window_handle().window_id(),
                        source: None,
                        value: Arc::new("drag"),
                        view: drag_view,
                        window_preview_offset: point(px(0.0), px(0.0)),
                        cursor_style: None,
                        button: MouseButton::Left,
                    });
                    let pointer_capture = view.read(cx).pointer_capture;
                    window
                        .capture_pointer(&pointer_capture, MouseButton::Left)
                        .expect("the interceptor probe should bind its pointer capture handle");
                });
            }

            let result = cx.update(|window, cx| window.dispatch_event(input, cx));
            assert_eq!(&*events.borrow(), &[interceptor]);
            assert!(!result.propagate);
            assert!(result.default_prevented);

            if interceptor == "interceptor-up" {
                cx.update(|window, cx| {
                    assert!(cx.active_drag.is_none());
                    assert!(window.captured_pointer().is_none());
                });
            }
        }
    }

    #[open_gpui::test]
    fn synchronous_mouse_reentrancy_is_rejected_before_interceptors(cx: &mut TestAppContext) {
        let consume = Rc::new(Cell::new(false));
        let reentered = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, _cx| {
            let events = view.events.clone();
            let reentered = reentered.clone();
            view.mouse_subscription = Some(window.intercept_window_mouse_events(
                move |event, window, cx| {
                    let WindowMouseEvent::Down(event) = event else {
                        return;
                    };
                    events.borrow_mut().push("interceptor");
                    if !reentered.replace(true) {
                        let nested =
                            window.dispatch_event(PlatformInput::MouseDown(event.clone()), cx);
                        assert!(!nested.propagate);
                        assert!(nested.default_prevented);
                        events.borrow_mut().push("nested-rejected");
                    }
                },
            ));
        });

        let result = cx.simulate_event_with_dispatch_snapshot(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(10.0), px(10.0)),
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        });
        assert_eq!(
            &*events.borrow(),
            &["interceptor", "nested-rejected", "node"]
        );
        assert!(result.propagated());
    }

    #[open_gpui::test]
    fn synchronous_mouse_to_key_reentrancy_preserves_outer_dispatch_state(cx: &mut TestAppContext) {
        let consume = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, _cx| {
            let events = view.events.clone();
            view.mouse_subscription = Some(window.intercept_window_mouse_events(
                move |event, window, cx| {
                    assert!(matches!(event, WindowMouseEvent::Move(_)));
                    events.borrow_mut().push("mouse-interceptor");
                    let nested = window.dispatch_event(
                        PlatformInput::KeyDown(KeyDownEvent {
                            keystroke: Keystroke::parse("escape").expect("escape should parse"),
                            is_held: false,
                            prefer_character_input: false,
                        }),
                        cx,
                    );
                    assert!(!nested.propagate);
                    assert!(nested.default_prevented);
                    events.borrow_mut().push("nested-rejected");
                },
            ));
        });

        let result = cx.simulate_event_with_dispatch_snapshot(MouseMoveEvent {
            position: point(px(10.0), px(10.0)),
            pressed_button: None,
            modifiers: Modifiers::none(),
        });
        assert_eq!(
            &*events.borrow(),
            &["mouse-interceptor", "nested-rejected", "node-move"]
        );
        assert!(result.propagated());
        assert!(!result.default_prevented());
    }

    #[open_gpui::test]
    fn window_key_interceptor_precedes_node_dispatch_and_can_consume(cx: &mut TestAppContext) {
        let consume = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, cx| {
            view.focus.focus(window, cx);
            let consume = view.consume.clone();
            let events = view.events.clone();
            view.key_subscription = Some(window.intercept_window_key_down(
                move |_: &KeyDownEvent, window, cx| {
                    events.borrow_mut().push("interceptor");
                    if consume.get() {
                        cx.stop_propagation();
                        window.prevent_default();
                    }
                },
            ));
        });

        let pass_through = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
            keystroke: Keystroke::parse("escape").expect("escape should parse"),
            is_held: false,
            prefer_character_input: false,
        });
        assert_eq!(
            &*events.borrow(),
            &["interceptor", "node-capture", "node-bubble"]
        );
        assert!(pass_through.propagated());

        events.borrow_mut().clear();
        consume.set(true);
        let consumed = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
            keystroke: Keystroke::parse("escape").expect("escape should parse"),
            is_held: false,
            prefer_character_input: false,
        });
        assert_eq!(&*events.borrow(), &["interceptor"]);
        assert!(consumed.propagation_stopped());
        assert!(consumed.default_prevented());
    }

    #[open_gpui::test]
    fn window_key_interceptor_replays_pending_printable_prefix_when_it_consumes_next_key(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            cx.bind_keys([KeyBinding::new("a b", PendingChordAction, None)]);
        });

        let consume = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, cx| {
            view.focus.focus(window, cx);
            let events = view.events.clone();
            view.key_subscription =
                Some(window.intercept_window_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        events.borrow_mut().push("interceptor-escape");
                        cx.stop_propagation();
                        window.prevent_default();
                    } else {
                        events.borrow_mut().push("interceptor-prefix");
                    }
                }));
        });

        let prefix = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
            keystroke: Keystroke::parse("a").expect("printable prefix should parse"),
            is_held: false,
            prefer_character_input: false,
        });
        assert!(prefix.propagation_stopped());
        cx.update(|window, _| assert!(window.has_pending_keystrokes()));

        events.borrow_mut().clear();
        let consumed = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
            keystroke: Keystroke::parse("escape").expect("escape should parse"),
            is_held: false,
            prefer_character_input: false,
        });
        assert_eq!(
            &*events.borrow(),
            &["interceptor-escape", "node-capture", "node-bubble"]
        );
        assert!(consumed.propagation_stopped());
        assert!(consumed.default_prevented());
        cx.update(|window, _| assert!(!window.has_pending_keystrokes()));
    }

    #[open_gpui::test]
    fn synchronous_key_reentrancy_is_rejected_before_interceptors(cx: &mut TestAppContext) {
        let consume = Rc::new(Cell::new(false));
        let reentered = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, cx| {
            view.focus.focus(window, cx);
            let events = view.events.clone();
            let reentered = reentered.clone();
            view.key_subscription =
                Some(window.intercept_window_key_down(move |event, window, cx| {
                    events.borrow_mut().push("interceptor");
                    if !reentered.replace(true) {
                        let nested =
                            window.dispatch_event(PlatformInput::KeyDown(event.clone()), cx);
                        assert!(!nested.propagate);
                        assert!(nested.default_prevented);
                        events.borrow_mut().push("nested-rejected");
                    }
                }));
        });

        let result = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
            keystroke: Keystroke::parse("escape").expect("escape should parse"),
            is_held: false,
            prefer_character_input: false,
        });
        assert_eq!(
            &*events.borrow(),
            &[
                "interceptor",
                "nested-rejected",
                "node-capture",
                "node-bubble",
            ]
        );
        assert!(result.propagated());
    }

    #[open_gpui::test]
    fn synchronous_key_to_mouse_reentrancy_preserves_outer_dispatch_state(cx: &mut TestAppContext) {
        let consume = Rc::new(Cell::new(false));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let consume = consume.clone();
            let events = events.clone();
            move |window, cx| WindowMouseInterceptorProbe {
                consume,
                events,
                focus: cx.focus_handle(),
                pointer_capture: window.new_pointer_capture_handle(),
                mouse_subscription: None,
                key_subscription: None,
            }
        });
        cx.update(|window, cx| window.draw(cx).clear());
        cx.update_window_entity(&view, |view, window, cx| {
            view.focus.focus(window, cx);
            let events = view.events.clone();
            view.key_subscription = Some(window.intercept_window_key_down(move |_, window, cx| {
                events.borrow_mut().push("key-interceptor");
                let nested = window.dispatch_event(
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: point(px(10.0), px(10.0)),
                        modifiers: Modifiers::none(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    cx,
                );
                assert!(!nested.propagate);
                assert!(nested.default_prevented);
                events.borrow_mut().push("nested-rejected");
            }));
        });

        let result = cx.simulate_event_with_dispatch_snapshot(KeyDownEvent {
            keystroke: Keystroke::parse("escape").expect("escape should parse"),
            is_held: false,
            prefer_character_input: false,
        });
        assert_eq!(
            &*events.borrow(),
            &[
                "key-interceptor",
                "nested-rejected",
                "node-capture",
                "node-bubble",
            ]
        );
        assert!(result.propagated());
        assert!(!result.default_prevented());
    }

    #[open_gpui::test]
    fn test_input_dispatch_snapshot_records_default_and_propagation(cx: &mut TestAppContext) {
        let (_view, cx) = cx.add_window_view(|_, _| InputDispatchProbe);

        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
        cx.clear_last_input_dispatch();
        assert_eq!(cx.last_input_dispatch(), None);

        let dispatch = cx.simulate_event_with_dispatch_snapshot(ScrollWheelEvent {
            position: point(px(10.0), px(10.0)),
            delta: ScrollDelta::Pixels(point(px(0.0), px(-24.0))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });

        assert_eq!(cx.last_input_dispatch(), Some(dispatch));
        assert!(dispatch.default_prevented());
        assert!(dispatch.default_consumed());
        assert!(dispatch.propagation_stopped());
        assert!(!dispatch.propagated());
    }

    #[open_gpui::test]
    async fn test_simulate_path_prompt_response(cx: &mut TestAppContext) {
        assert!(!cx.did_prompt_for_paths());

        let receiver = cx.update(|cx| {
            cx.prompt_for_paths(PathPromptOptions {
                files: false,
                directories: true,
                multiple: true,
                prompt: None,
            })
        });
        assert!(cx.did_prompt_for_paths());

        let selected = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        cx.simulate_path_prompt_response({
            let selected = selected.clone();
            move |options| {
                assert!(options.multiple);
                Some(selected)
            }
        });
        assert!(!cx.did_prompt_for_paths());

        let response = receiver.await.unwrap().unwrap();
        assert_eq!(response, Some(selected));
    }

    #[open_gpui::test]
    async fn test_simulate_path_prompt_cancellation(cx: &mut TestAppContext) {
        let receiver = cx.update(|cx| {
            cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: None,
            })
        });

        cx.simulate_path_prompt_response(|_options| None);

        let response = receiver.await.unwrap().unwrap();
        assert_eq!(response, None);
    }

    #[open_gpui::test]
    fn test_platform_hovered_window_signal(cx: &mut TestAppContext) {
        let first = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let second = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();

        cx.set_platform_hovered_window(Some(second));
        let hovered = cx.update(|app| app.hovered_window());

        assert_eq!(hovered, PlatformHoveredWindow::Window(second));
        assert_eq!(hovered.window(), Some(second));
        assert!(hovered.is_available());

        cx.set_platform_hovered_window(Some(first));
        assert_eq!(
            cx.update(|app| app.hovered_window()),
            PlatformHoveredWindow::Window(first)
        );

        cx.set_platform_hovered_window(None);
        assert_eq!(
            cx.update(|app| app.hovered_window()),
            PlatformHoveredWindow::NoWindow
        );

        cx.set_platform_hovered_window(Some(first));
        cx.set_platform_hovered_window_available(false);
        let hovered = cx.update(|app| app.hovered_window());
        assert_eq!(hovered, PlatformHoveredWindow::Unavailable);
        assert!(!hovered.is_available());

        cx.set_platform_hovered_window_available(true);
        assert_eq!(
            cx.update(|app| app.hovered_window()),
            PlatformHoveredWindow::Window(first)
        );
    }

    #[open_gpui::test]
    fn test_platform_window_hit_stack_is_scoped_to_the_sampled_physical_point(
        cx: &mut TestAppContext,
    ) {
        let registered = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let sampled_point = point(DevicePixels(-48), DevicePixels(96));
        let hits = vec![PlatformWindowHit::RegisteredApplication {
            window: registered,
            coverage: PlatformWindowPhysicalCoverage::try_new(Bounds::new(
                point(DevicePixels(-64), DevicePixels(80)),
                size(DevicePixels(640), DevicePixels(400)),
            ))
            .expect("test coverage must be representable"),
            geometry: PlatformWindowPhysicalGeometry::try_new(
                Bounds::new(
                    point(DevicePixels(-48), DevicePixels(96)),
                    size(DevicePixels(608), DevicePixels(352)),
                ),
                1.5,
            )
            .expect("test geometry must be representable"),
        }];
        let stack = PlatformWindowHitStack::try_available(sampled_point, hits)
            .expect("test hits must cover the sampled point");

        assert!(!cx.update(|app| app.viewport_capabilities().window_hit_stack));

        cx.set_platform_window_hit_stack(stack.clone());

        assert!(cx.update(|app| app.viewport_capabilities().window_hit_stack));
        assert_eq!(cx.test_platform.window_hit_stack_at(sampled_point), stack);
        assert_eq!(
            cx.test_platform
                .window_hit_stack_at(point(DevicePixels(-47), DevicePixels(96))),
            PlatformWindowHitStack::Unavailable,
            "a hit stack sampled at another physical point must not be reused"
        );
    }

    #[open_gpui::test]
    fn test_platform_window_hit_stack_distinguishes_unavailable_from_no_hits(
        cx: &mut TestAppContext,
    ) {
        let sampled_point = point(DevicePixels(12), DevicePixels(-24));

        assert_eq!(
            cx.test_platform.window_hit_stack_at(sampled_point),
            PlatformWindowHitStack::Unavailable
        );

        let no_hits = PlatformWindowHitStack::try_available_open_desktop(sampled_point, Vec::new())
            .expect("an empty observation is a valid desktop result");
        cx.set_platform_window_hit_stack(no_hits.clone());

        assert_eq!(cx.test_platform.window_hit_stack_at(sampled_point), no_hits);
    }

    #[open_gpui::test]
    fn test_simulate_window_close_honors_last_window_quit_mode(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_quit_mode(QuitMode::LastWindowClosed));
        let window = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();

        assert!(!cx.did_quit());
        assert!(cx.simulate_window_close(window));
        assert!(cx.windows().is_empty());
        cx.run_until_parked();
        assert!(cx.did_quit());
        assert!(cx.update(|app| app.native_exit_authority_is_settled_for_test()));
    }

    #[open_gpui::test]
    fn simulate_window_close_tolerates_handler_removing_window(cx: &mut TestAppContext) {
        let window = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let platform_window = cx.test_window(window);
        let close_count = Rc::new(Cell::new(0usize));
        cx.update(|app| {
            let close_count = close_count.clone();
            app.on_window_closed(move |_, closed_window| {
                assert_eq!(closed_window, window.window_id());
                close_count.set(close_count.get() + 1);
            })
            .detach();
        });
        cx.update_window(window, |_, window, app| {
            window.on_window_should_close(app, |window, app| {
                window.remove_window(app);
                true
            });
        })
        .expect("the window must exist before its close request");

        assert!(cx.simulate_window_close(window));
        assert!(cx.windows().is_empty());
        assert_eq!(close_count.get(), 1);
        assert!(
            !platform_window.simulate_close(),
            "the native close callback must be consumed exactly once"
        );
    }

    #[open_gpui::test]
    fn visual_simulate_close_tolerates_handler_removing_window(cx: &mut TestAppContext) {
        let window = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        let platform_window = cx.test_window(window);
        let close_count = Rc::new(Cell::new(0usize));
        cx.update(|app| {
            let close_count = close_count.clone();
            app.on_window_closed(move |_, closed_window| {
                assert_eq!(closed_window, window.window_id());
                close_count.set(close_count.get() + 1);
            })
            .detach();
        });
        cx.update_window(window, |_, window, app| {
            window.on_window_should_close(app, |window, app| {
                window.remove_window(app);
                true
            });
        })
        .expect("the window must exist before its close request");
        let mut visual = VisualTestContext::from_window(window, cx);

        assert!(visual.simulate_close());
        assert!(visual.windows().is_empty());
        assert_eq!(close_count.get(), 1);
        assert!(
            !platform_window.simulate_close(),
            "the native close callback must be consumed exactly once"
        );
    }

    #[open_gpui::test]
    fn test_input_dispatch_snapshot_records_simulated_input(cx: &mut TestAppContext) {
        let window = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
            .into();
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();

        cx.clear_last_input_dispatch();
        let mut visual = VisualTestContext::from_window(window, cx);
        let snapshot = visual.simulate_event_with_dispatch_snapshot(MouseDownEvent {
            position: point(px(16.0), px(16.0)),
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        });

        assert_eq!(snapshot, TestInputDispatchSnapshot::default());
        assert_eq!(visual.last_input_dispatch(), Some(snapshot));
        assert_eq!(
            visual
                .last_dispatch_event_result()
                .map(TestInputDispatchSnapshot::from),
            Some(snapshot)
        );
    }

    #[open_gpui::test]
    fn cursor_style_uses_platform_hovered_window_not_active_window(cx: &mut TestAppContext) {
        let active = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| CursorProbeView {
                pointer: false,
            })
            .into();
        let hovered = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| CursorProbeView {
                pointer: true,
            })
            .into();

        cx.update_window(active, |_, window, _| window.activate_window())
            .unwrap();
        cx.run_until_parked();
        cx.set_platform_hovered_window(Some(hovered));
        cx.run_until_parked();

        cx.update_window(hovered, |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();
        cx.update_window(active, |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();

        assert_eq!(cx.platform_cursor_style(), CursorStyle::PointingHand);
    }

    #[open_gpui::test]
    fn mouse_exit_clears_window_cursor_style(cx: &mut TestAppContext) {
        let window_handle = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| WindowCursorProbeView {
                style: CursorStyle::ResizeColumn,
            })
            .into();

        cx.set_platform_hovered_window(Some(window_handle));
        cx.update_window(window_handle, |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();
        assert_eq!(cx.platform_cursor_style(), CursorStyle::ResizeColumn);

        let mut visual = VisualTestContext::from_window(window_handle, cx);
        visual.simulate_mouse_exit(point(px(400.0), px(220.0)), None, Modifiers::none());

        assert_eq!(cx.platform_cursor_style(), CursorStyle::Arrow);
    }

    #[open_gpui::test]
    fn active_drag_cursor_is_only_applied_by_hovered_window(cx: &mut TestAppContext) {
        let hovered = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| CursorProbeView {
                pointer: true,
            })
            .into();
        let background = cx
            .open_window(size(px(320.0), px(200.0)), |_, _| CursorProbeView {
                pointer: true,
            })
            .into();

        cx.set_platform_hovered_window(Some(hovered));
        let mut visual = VisualTestContext::from_window(hovered, cx);
        visual.simulate_mouse_move(point(px(16.0), px(16.0)), None, Modifiers::none());
        cx.update(|app| {
            app.active_drag = Some(AnyDrag {
                window_id: hovered.window_id(),
                source: None,
                value: Arc::new("drag"),
                view: app.new(|_| Empty).into(),
                window_preview_offset: point(px(0.0), px(0.0)),
                cursor_style: Some(CursorStyle::ClosedHand),
                button: MouseButton::Left,
            });
        });

        cx.update_window(hovered, |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();
        assert_eq!(cx.platform_cursor_style(), CursorStyle::ClosedHand);

        cx.update_window(background, |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();
        assert_eq!(
            cx.platform_cursor_style(),
            CursorStyle::ClosedHand,
            "a non-hovered window repaint must not overwrite the active drag cursor"
        );
    }

    #[open_gpui::test]
    fn test_focused_debug_selector_tracks_rendered_focus(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| FocusDebugView {
            first: cx.focus_handle(),
            second: cx.focus_handle(),
        });

        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert_eq!(cx.focused_debug_selector(), None);
        assert!(!cx.debug_selector_is_focused("focus-debug:first"));

        cx.update_window_entity(&view, |view, window, cx| view.first.focus(window, cx));
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert_eq!(
            cx.focused_debug_selector().as_deref(),
            Some("focus-debug:first")
        );
        assert!(cx.debug_selector_is_focused("focus-debug:first"));
        assert!(!cx.debug_selector_is_focused("focus-debug:second"));

        cx.update_window_entity(&view, |view, window, cx| view.second.focus(window, cx));
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
        assert_eq!(
            cx.focused_debug_selector().as_deref(),
            Some("focus-debug:second")
        );
        assert!(cx.debug_selector_is_focused("focus-debug:second"));
        assert!(!cx.debug_selector_is_focused("focus-debug:first"));
    }

    #[open_gpui::test]
    fn test_simulate_drag_dispatches_mouse_sequence(cx: &mut TestAppContext) {
        use std::cell::RefCell;
        use std::rc::Rc;

        #[derive(Clone)]
        struct DragRecorder {
            events: Rc<RefCell<Vec<&'static str>>>,
        }

        impl Render for DragRecorder {
            fn render(
                &mut self,
                _window: &mut crate::Window,
                _cx: &mut Context<Self>,
            ) -> impl IntoElement {
                div()
                    .id("drag-recorder")
                    .w(px(200.0))
                    .h(px(120.0))
                    .on_mouse_down(MouseButton::Left, {
                        let events = self.events.clone();
                        move |_, _, _| events.borrow_mut().push("down")
                    })
                    .on_mouse_move({
                        let events = self.events.clone();
                        move |_, _, _| events.borrow_mut().push("move")
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let events = self.events.clone();
                        move |_, _, _| events.borrow_mut().push("up")
                    })
            }
        }

        let events = Rc::new(RefCell::new(Vec::new()));
        let events_for_view = events.clone();
        let (_view, visual_cx) = cx.add_window_view(move |_, _| DragRecorder {
            events: events_for_view.clone(),
        });

        visual_cx.simulate_drag(
            point(px(20.0), px(20.0)),
            point(px(120.0), px(80.0)),
            MouseButton::Left,
            crate::Modifiers::none(),
        );

        assert_eq!(events.borrow().as_slice(), ["down", "move", "up"]);
    }
}
