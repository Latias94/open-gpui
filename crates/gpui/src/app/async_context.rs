use crate::{
    AnyView, AnyWindowHandle, App, AppCell, AppContext, BackgroundExecutor, BorrowAppContext,
    DispatchEventResult, Entity, EntityId, EventEmitter, Focusable, ForegroundExecutor, Global,
    GpuiBorrow, ModifiersChangedEvent, NativeInputInvariantViolation, PlatformInput,
    PlatformWindowMutationObservation, PromptButton, PromptLevel, Render, RequestFrameOptions,
    Reservation, Result, Subscription, Task, VisualContext, Window, WindowControlArea,
    WindowHandle, WindowId,
};
use anyhow::{Context as _, bail};
use derive_more::{Deref, DerefMut};
use futures::channel::oneshot;
use futures::future::FutureExt;
use std::{future::Future, rc::Weak};

use super::{Context, WeakEntity};

/// An async-friendly version of [App] with a static lifetime so it can be held across `await` points in async code.
/// You're provided with an instance when calling [App::spawn], and you can also create one with [App::to_async].
///
/// Internally, this holds a weak reference to an `App`. Methods will panic if the app has been dropped,
/// but this should not happen in practice when using foreground tasks spawned via `cx.spawn()`,
/// as the executor checks if the app is alive before running each task.
#[derive(Clone)]
pub struct AsyncApp {
    pub(crate) app: Weak<AppCell>,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
}

impl AsyncApp {
    fn app(&self) -> std::rc::Rc<AppCell> {
        self.app
            .upgrade()
            .expect("app was released before async operation completed")
    }

    pub(crate) fn enqueue_window_mutation_observation(
        &self,
        window_id: WindowId,
        observation: PlatformWindowMutationObservation,
    ) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::WindowMutationObserved(observation),
        );
    }

    fn enqueue_native_window_event(
        &self,
        window_id: WindowId,
        event: super::native_event_ingress::NativeWindowEvent,
    ) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.enqueue_native_window_event(window_id, event);
    }

    pub(crate) fn enqueue_window_resized(&self, window_id: WindowId) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::Resized,
        );
    }

    pub(crate) fn enqueue_window_moved(&self, window_id: WindowId) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::Moved,
        );
    }

    pub(crate) fn enqueue_window_state_changed(&self, window_id: WindowId) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::WindowStateChanged,
        );
    }

    pub(crate) fn enqueue_window_active_changed(&self, window_id: WindowId, active: bool) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::ActiveChanged(active),
        );
    }

    pub(crate) fn enqueue_window_modifiers_changed(
        &self,
        window_id: WindowId,
        event: ModifiersChangedEvent,
    ) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::ModifiersChanged(event),
        );
    }

    pub(crate) fn enqueue_window_hover_changed(&self, window_id: WindowId, hovered: bool) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::HoverChanged(hovered),
        );
    }

    pub(crate) fn enqueue_window_appearance_changed(&self, window_id: WindowId) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::AppearanceChanged,
        );
    }

    pub(crate) fn enqueue_window_button_layout_changed(&self, window_id: WindowId) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::ButtonLayoutChanged,
        );
    }

    pub(crate) fn enqueue_window_frame_requested(
        &self,
        window_id: WindowId,
        options: RequestFrameOptions,
    ) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::RequestFrame(options),
        );
    }

    fn enqueue_system_tab_command(
        &self,
        window_id: WindowId,
        command: super::native_event_ingress::NativeSystemTabCommand,
    ) {
        self.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::SystemTabCommand(command),
        );
    }

    pub(crate) fn enqueue_move_tab_to_new_window(&self, window_id: WindowId) {
        self.enqueue_system_tab_command(
            window_id,
            super::native_event_ingress::NativeSystemTabCommand::MoveToNewWindow,
        );
    }

    pub(crate) fn enqueue_merge_all_windows(&self, window_id: WindowId) {
        self.enqueue_system_tab_command(
            window_id,
            super::native_event_ingress::NativeSystemTabCommand::MergeAll,
        );
    }

    pub(crate) fn enqueue_select_next_tab(&self, window_id: WindowId) {
        self.enqueue_system_tab_command(
            window_id,
            super::native_event_ingress::NativeSystemTabCommand::SelectNext,
        );
    }

    pub(crate) fn enqueue_select_previous_tab(&self, window_id: WindowId) {
        self.enqueue_system_tab_command(
            window_id,
            super::native_event_ingress::NativeSystemTabCommand::SelectPrevious,
        );
    }

    pub(crate) fn enqueue_toggle_tab_bar(&self, window_id: WindowId) {
        self.enqueue_system_tab_command(
            window_id,
            super::native_event_ingress::NativeSystemTabCommand::ToggleBar,
        );
    }

    pub(crate) fn enqueue_window_closed(&self, window_id: WindowId) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.enqueue_native_window_event(
            window_id,
            super::native_event_ingress::NativeWindowEvent::Closed,
        );
    }

    pub(crate) fn dispatch_window_should_close(&self, window_id: WindowId) -> bool {
        let Some(app) = self.app.upgrade() else {
            return true;
        };
        app.dispatch_window_should_close(window_id)
    }

    pub(crate) fn dispatch_native_window_input(
        &self,
        window_id: WindowId,
        event: PlatformInput,
    ) -> std::result::Result<DispatchEventResult, NativeInputInvariantViolation> {
        let app = self
            .app
            .upgrade()
            .expect("platform input callback lease must retain AppCell");
        app.dispatch_native_window_input(window_id, event)
    }

    pub(crate) fn begin_platform_input_lease(
        &self,
        generation: u64,
    ) -> Option<super::cell::NativeCallbackLease> {
        self.app
            .upgrade()
            .map(|app| app.begin_platform_input_lease(generation))
    }

    pub(crate) fn native_window_control_area(
        &self,
        window_id: WindowId,
    ) -> Option<WindowControlArea> {
        self.app
            .upgrade()
            .and_then(|app| app.native_window_control_area(window_id))
    }
}

impl AppContext for AsyncApp {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.new(build_entity)
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.insert_entity(reservation, build_entity)
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, _handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot as_mut with an async context. Try calling update() first")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, callback: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let app = self.app();
        let lock = app.borrow();
        lock.read_entity(handle, callback)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let app = self.app.upgrade().context("app was released")?;
        let mut lock = app.try_borrow_mut()?;
        if lock.quitting {
            bail!("app is quitting");
        }
        lock.update_window(window, f)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        let app = self.app.upgrade()?;
        let mut lock = app.try_borrow_mut().ok()?;
        if lock.quitting {
            return None;
        }
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
        let app = self.app.upgrade().context("app was released")?;
        let lock = app.borrow();
        if lock.quitting {
            bail!("app is quitting");
        }
        lock.read_window(window, read)
    }

    #[track_caller]
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
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.update(|this| this.read_global(callback))
    }
}

impl AsyncApp {
    /// Schedules all windows in the application to be redrawn.
    pub fn refresh(&self) {
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.refresh_windows();
    }

    /// Get an executor which can be used to spawn futures in the background.
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// Get an executor which can be used to spawn futures in the foreground.
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }

    /// Invoke the given function in the context of the app, then flush any effects produced during its invocation.
    pub fn update<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.update(f)
    }

    /// Arrange for the given callback to be invoked whenever the given entity emits an event of a given type.
    /// The callback is provided a handle to the emitting entity and a reference to the emitted event.
    pub fn subscribe<T, Event>(
        &mut self,
        entity: &Entity<T>,
        on_event: impl FnMut(Entity<T>, &Event, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Event>,
        Event: 'static,
    {
        let app = self.app();
        let mut lock = app.borrow_mut();
        lock.subscribe(entity, on_event)
    }

    /// Open a window with the given options based on the root view returned by the given function.
    pub fn open_window<V>(
        &self,
        options: crate::WindowOptions,
        build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>>
    where
        V: 'static + Render,
    {
        let app = self.app();
        let mut lock = app.borrow_mut();
        if lock.quitting {
            bail!("app is quitting");
        }
        lock.open_window(options, build_root_view)
    }

    /// Schedule a future to be polled in the foreground.
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        let mut cx = self.clone();
        self.foreground_executor
            .spawn(async move { f(&mut cx).await }.boxed_local())
    }

    /// Determine whether global state of the specified type has been assigned.
    pub fn has_global<G: Global>(&self) -> bool {
        let app = self.app();
        let app = app.borrow_mut();
        app.has_global::<G>()
    }

    /// Reads the global state of the specified type, passing it to the given callback.
    ///
    /// Panics if no global state of the specified type has been assigned.
    pub fn read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> R {
        let app = self.app();
        let app = app.borrow_mut();
        read(app.global(), &app)
    }

    /// Reads the global state of the specified type, passing it to the given callback.
    ///
    /// Similar to [`AsyncApp::read_global`], but returns an error instead of panicking
    pub fn try_read_global<G: Global, R>(&self, read: impl FnOnce(&G, &App) -> R) -> Option<R> {
        let app = self.app();
        let app = app.borrow_mut();
        if app.quitting {
            return None;
        }
        Some(read(app.try_global()?, &app))
    }

    /// Reads the global state of the specified type, passing it to the given callback.
    /// A default value is assigned if a global of this type has not yet been assigned.
    pub fn read_default_global<G: Global + Default, R>(
        &self,
        read: impl FnOnce(&G, &App) -> R,
    ) -> R {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.update(|cx| {
            cx.default_global::<G>();
        });
        read(app.global(), &app)
    }

    /// A convenience method for [`App::update_global`](BorrowAppContext::update_global)
    /// for updating the global state of the specified type.
    pub fn update_global<G: Global, R>(&self, update: impl FnOnce(&mut G, &mut App) -> R) -> R {
        let app = self.app();
        let mut app = app.borrow_mut();
        app.update(|cx| cx.update_global(update))
    }

    /// Run something using this entity and cx, when the returned struct is dropped
    pub fn on_drop<T: 'static, Callback: FnOnce(&mut T, &mut Context<T>) + 'static>(
        &self,
        entity: &WeakEntity<T>,
        f: Callback,
    ) -> open_gpui_core_util::Deferred<impl FnOnce() + use<T, Callback>> {
        let entity = entity.clone();
        let mut cx = self.clone();
        open_gpui_core_util::defer(move || {
            entity.update(&mut cx, f).ok();
        })
    }
}

/// A cloneable, owned handle to the application context,
/// composed with the window associated with the current task.
#[derive(Clone, Deref, DerefMut)]
pub struct AsyncWindowContext {
    #[deref]
    #[deref_mut]
    app: AsyncApp,
    window: AnyWindowHandle,
}

impl AsyncWindowContext {
    pub(crate) fn new_context(app: AsyncApp, window: AnyWindowHandle) -> Self {
        Self { app, window }
    }

    /// Get the handle of the window this context is associated with.
    pub fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    /// A convenience method for [`App::update_window`].
    pub fn update<R>(&mut self, update: impl FnOnce(&mut Window, &mut App) -> R) -> Result<R> {
        self.app
            .update_window(self.window, |_, window, cx| update(window, cx))
    }

    pub(crate) fn update_native_input_handler<R>(
        &mut self,
        operation: crate::NativeInputHandlerOperation,
        update: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> std::result::Result<R, NativeInputInvariantViolation> {
        let app = self
            .app
            .app
            .upgrade()
            .expect("input-handler callback lease must retain AppCell");
        app.dispatch_native_input_handler(self.window.window_id(), operation, update)
    }

    pub(crate) fn begin_input_handler_lease(
        &self,
        generation: u64,
    ) -> Option<super::cell::NativeCallbackLease> {
        self.app
            .app
            .upgrade()
            .map(|app| app.begin_input_handler_lease(generation))
    }

    pub(crate) fn native_input_diagnostic_target(&self) -> (Weak<AppCell>, WindowId) {
        (self.app.app.clone(), self.window.window_id())
    }

    /// A convenience method for [`App::update_window`].
    pub fn update_root<R>(
        &mut self,
        update: impl FnOnce(AnyView, &mut Window, &mut App) -> R,
    ) -> Result<R> {
        self.app.update_window(self.window, update)
    }

    /// A convenience method for [`Window::on_next_frame`].
    pub fn on_next_frame(&mut self, f: impl FnOnce(&mut Window, &mut App) + 'static) {
        self.app
            .update_window(self.window, |_, window, _| window.on_next_frame(f))
            .ok();
    }

    /// A convenience method for [`App::global`].
    pub fn read_global<G: Global, R>(
        &mut self,
        read: impl FnOnce(&G, &Window, &App) -> R,
    ) -> Result<R> {
        self.app
            .update_window(self.window, |_, window, cx| read(cx.global(), window, cx))
    }

    /// A convenience method for [`App::update_global`](BorrowAppContext::update_global).
    /// for updating the global state of the specified type.
    pub fn update_global<G, R>(
        &mut self,
        update: impl FnOnce(&mut G, &mut Window, &mut App) -> R,
    ) -> Result<R>
    where
        G: Global,
    {
        self.app.update_window(self.window, |_, window, cx| {
            cx.update_global(|global, cx| update(global, window, cx))
        })
    }

    /// Schedule a future to be executed on the main thread. This is used for collecting
    /// the results of background tasks and updating the UI.
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
        R: 'static,
    {
        let mut cx = self.clone();
        self.foreground_executor
            .spawn(async move { f(&mut cx).await }.boxed_local())
    }

    /// Present a platform dialog.
    /// The provided message will be presented, along with buttons for each answer.
    /// When a button is clicked, the returned Receiver will receive the index of the clicked button.
    pub fn prompt<T>(
        &mut self,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[T],
    ) -> oneshot::Receiver<usize>
    where
        T: Clone + Into<PromptButton>,
    {
        self.app
            .update_window(self.window, |_, window, cx| {
                window.prompt(level, message, detail, answers, cx)
            })
            .unwrap_or_else(|_| oneshot::channel().1)
    }
}

impl AppContext for AsyncWindowContext {
    fn new<T>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T>
    where
        T: 'static,
    {
        let mut build_entity = Some(build_entity);
        match self.app.update_window(self.window, |_, _, cx| {
            cx.new(
                build_entity
                    .take()
                    .expect("build_entity is taken exactly once"),
            )
        }) {
            Ok(entity) => entity,
            Err(_) => self.app.new(
                build_entity
                    .take()
                    .expect("update_window returned Err without invoking the closure"),
            ),
        }
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        self.app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let mut args = Some((reservation, build_entity));
        match self.app.update_window(self.window, |_, _, cx| {
            let (reservation, build_entity) = args.take().expect("args are taken exactly once");
            cx.insert_entity(reservation, build_entity)
        }) {
            Ok(entity) => entity,
            Err(_) => {
                let (reservation, build_entity) = args
                    .take()
                    .expect("update_window returned Err without invoking the closure");
                self.app.insert_entity(reservation, build_entity)
            }
        }
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        self.app.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut() from an async context, call `update`")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        self.app.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.app.update_window(window, update)
    }

    fn with_window<R>(
        &mut self,
        entity_id: EntityId,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> Option<R> {
        self.app.with_window(entity_id, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        self.app.read_window(window, read)
    }

    #[track_caller]
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.app.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        self.app.read_global(callback)
    }
}

impl VisualContext for AsyncWindowContext {
    type Result<T> = Result<T>;

    fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Result<Entity<T>> {
        self.app.update_window(self.window, |_, window, cx| {
            cx.new(|cx| build_entity(window, cx))
        })
    }

    fn update_window_entity<T: 'static, R>(
        &mut self,
        view: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Result<R> {
        let view = view.clone();
        self.app
            .with_window(view.entity_id(), |window, app| {
                view.update(app, |entity, cx| update(entity, window, cx))
            })
            .context("entity has no current window")
    }

    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Result<Entity<V>>
    where
        V: 'static + Render,
    {
        self.app.update_window(self.window, |_, window, cx| {
            window.replace_root(cx, build_view)
        })
    }

    fn focus<V>(&mut self, view: &Entity<V>) -> Result<()>
    where
        V: Focusable,
    {
        self.app.update_window(self.window, |_, window, cx| {
            view.read(cx).focus_handle(cx).focus(window, cx);
        })
    }
}
