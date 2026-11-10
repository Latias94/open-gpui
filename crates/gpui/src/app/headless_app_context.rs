//! Cross-platform headless app context for tests that need real text shaping.
//!
//! This replaces the macOS-only `HeadlessMetalAppContext` with a platform-neutral
//! implementation backed by `TestPlatform`. Tests supply a real `PlatformTextSystem`
//! (e.g. `DirectWriteTextSystem` on Windows, `AppleTextSystem` on Apple platforms) to get
//! accurate glyph measurements while keeping everything else deterministic.
//!
//! Optionally, a renderer factory can be provided to enable real GPU rendering
//! and screenshot capture via [`HeadlessAppContext::capture_screenshot`].

use crate::{
    AnyView, AnyWindowHandle, App, AppCell, AppContext, AssetSource, BackgroundExecutor, Bounds,
    Context, Entity, EntityId, ForegroundExecutor, Global, Pixels, PlatformHeadlessRenderer,
    PlatformTextSystem, Render, Reservation, Size, Task, TestDispatcher, TestPlatform, TextSystem,
    Window, WindowBounds, WindowFrameCapture, WindowHandle, WindowOptions,
    app::{GpuiBorrow, GpuiMode},
};
use anyhow::Result;
use image::RgbaImage;
use std::{future::Future, rc::Rc, sync::Arc, time::Duration};

/// A cross-platform headless app context for tests that need real text shaping.
///
/// Unlike the old `HeadlessMetalAppContext`, this works on any platform. It uses
/// `TestPlatform` for deterministic scheduling and accepts a pluggable
/// `PlatformTextSystem` so tests get real glyph measurements.
///
/// # Usage
///
/// ```ignore
/// let text_system = Arc::new(open_gpui_wgpu::CosmicTextSystem::new("fallback"));
/// let mut cx = HeadlessAppContext::with_platform(
///     text_system,
///     Arc::new(Assets),
///     || open_gpui_platform::current_headless_renderer(),
/// );
/// ```
pub struct HeadlessAppContext {
    /// The underlying app cell.
    pub app: Rc<AppCell>,
    /// The background executor for running async tasks.
    pub background_executor: BackgroundExecutor,
    /// The foreground executor for running tasks on the main thread.
    pub foreground_executor: ForegroundExecutor,
    dispatcher: TestDispatcher,
    text_system: Arc<TextSystem>,
}

impl HeadlessAppContext {
    /// Creates a new headless app context with the given text system.
    pub fn new(platform_text_system: Arc<dyn PlatformTextSystem>) -> Self {
        Self::with_platform(platform_text_system, Arc::new(()), || Ok(None))
    }

    /// Creates a new headless app context with a custom text system and asset source.
    pub fn with_asset_source(
        platform_text_system: Arc<dyn PlatformTextSystem>,
        asset_source: Arc<dyn AssetSource>,
    ) -> Self {
        Self::with_platform(platform_text_system, asset_source, || Ok(None))
    }

    /// Creates a new headless app context with the given text system, asset source,
    /// and a fallible renderer factory for GPU rendering and screenshot support.
    pub fn with_platform(
        platform_text_system: Arc<dyn PlatformTextSystem>,
        asset_source: Arc<dyn AssetSource>,
        renderer_factory: impl Fn() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>> + 'static,
    ) -> Self {
        let seed = std::env::var("SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let dispatcher = TestDispatcher::new(seed);
        let arc_dispatcher = Arc::new(dispatcher.clone());
        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);

        let renderer_factory: Box<dyn Fn() -> Result<Option<Box<dyn PlatformHeadlessRenderer>>>> =
            Box::new(renderer_factory);
        let platform = TestPlatform::with_platform(
            background_executor.clone(),
            foreground_executor.clone(),
            platform_text_system,
            Some(renderer_factory),
        );

        let http_client = open_gpui_http_client::FakeHttpClient::with_404_response();
        let app = App::new_app(platform, asset_source, http_client);
        app.borrow_mut().mode = GpuiMode::test();
        let text_system = app.borrow().text_system().clone();

        Self {
            app,
            background_executor,
            foreground_executor,
            dispatcher,
            text_system,
        }
    }

    /// Opens a window for headless rendering.
    pub fn open_window<V: Render + 'static>(
        &mut self,
        size: Size<Pixels>,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>> {
        use crate::{point, px};

        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size,
        };

        let mut cx = self.app.borrow_mut();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus_on_appearing: false,
                show: false,
                ..Default::default()
            },
            build_root,
        )
    }

    /// Runs all pending tasks until parked.
    pub fn run_until_parked(&self) {
        self.dispatcher.run_until_parked();
    }

    /// Delivers one scheduled frame or compositor callback and drains pending deterministic work.
    pub fn simulate_window_scheduled_frame(&self, window: AnyWindowHandle) -> Result<bool> {
        let platform_window = self
            .app
            .borrow_mut()
            .update_window(window, |_, window, _| {
                window
                    .platform_window
                    .as_test()
                    .map(|window| window.clone())
            })?
            .ok_or_else(|| anyhow::anyhow!("headless window is not backed by TestPlatform"))?;
        let app_borrow = self.app.borrow();
        let delivered = platform_window.simulate_scheduled_frame();
        drop(app_borrow);
        self.run_until_parked();
        Ok(delivered)
    }

    /// Advances the simulated clock.
    pub fn advance_clock(&self, duration: Duration) {
        self.dispatcher.advance_clock(duration);
    }

    /// Enables parking mode, allowing blocking on real I/O (e.g., async asset loading).
    pub fn allow_parking(&self) {
        self.dispatcher.allow_parking();
    }

    /// Disables parking mode, returning to deterministic test execution.
    pub fn forbid_parking(&self) {
        self.dispatcher.forbid_parking();
    }

    /// Updates app state.
    pub fn update<R>(&mut self, f: impl FnOnce(&mut App) -> R) -> R {
        let mut app = self.app.borrow_mut();
        f(&mut app)
    }

    /// Updates a window and calls draw to render.
    pub fn update_window<R>(
        &mut self,
        window: AnyWindowHandle,
        f: impl FnOnce(AnyView, &mut Window, &mut App) -> R,
    ) -> Result<R> {
        let mut app = self.app.borrow_mut();
        app.update_window(window, f)
    }

    /// Captures a screenshot from a window.
    ///
    /// Requires that the context was created with a renderer factory that
    /// returns `Some` via [`HeadlessAppContext::with_platform`].
    pub fn capture_screenshot(&mut self, window: AnyWindowHandle) -> Result<RgbaImage> {
        let mut app = self.app.borrow_mut();
        app.update_window(window, |_, window, _| window.render_to_image())?
    }

    /// Captures a screenshot together with framework frame metadata.
    pub fn capture_frame(&mut self, window: AnyWindowHandle) -> Result<WindowFrameCapture> {
        let mut app = self.app.borrow_mut();
        app.update_window(window, |_, window, _| window.capture_frame())
    }

    /// Returns the text system.
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// Returns the background executor.
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// Returns the foreground executor.
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }
}

impl Drop for HeadlessAppContext {
    fn drop(&mut self) {
        // Shut down the app so windows are closed and entity handles are
        // released before the LeakDetector runs.
        self.app.borrow_mut().shutdown();
    }
}

impl AppContext for HeadlessAppContext {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.new(build_entity)
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        let mut app = self.app.borrow_mut();
        app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
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

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut with HeadlessAppContext. Call update() instead.")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Empty, NoopTextSystem, px, size};
    use std::cell::Cell;

    #[test]
    fn renderer_factory_failure_precedes_root_creation_and_releases_window_reservation() {
        let fail = Rc::new(Cell::new(true));
        let factory_calls = Rc::new(Cell::new(0));
        let built_root = Rc::new(Cell::new(false));
        let mut cx = HeadlessAppContext::with_platform(Arc::new(NoopTextSystem), Arc::new(()), {
            let fail = fail.clone();
            let factory_calls = factory_calls.clone();
            move || {
                factory_calls.set(factory_calls.get() + 1);
                anyhow::ensure!(!fail.get(), "injected renderer factory failure");
                Ok(None)
            }
        });
        let result = cx.open_window(size(px(32.0), px(24.0)), {
            let built_root = built_root.clone();
            move |_, app| {
                built_root.set(true);
                app.new(|_| Empty)
            }
        });
        let error = result.unwrap_err();
        assert!(format!("{error:#}").contains("injected renderer factory failure"));
        assert_eq!(factory_calls.get(), 1);
        assert!(!built_root.get());
        assert!(cx.update(|app| app.windows().is_empty()));

        fail.set(false);
        let handle = cx
            .open_window(size(px(32.0), px(24.0)), |_, app| app.new(|_| Empty))
            .unwrap();
        assert_eq!(factory_calls.get(), 2);
        assert_eq!(cx.update(|app| app.windows()), vec![handle.into()]);
        cx.update_window(handle.into(), |_, window, app| window.remove_window(app))
            .unwrap();
        assert!(cx.update(|app| app.windows().is_empty()));
    }
}
