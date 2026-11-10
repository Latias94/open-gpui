use crate::dispatcher::{WebDispatcher, WebDispatcherMode};
use crate::display::WebDisplay;
use crate::http_client::FetchHttpClient;
use crate::keyboard::WebKeyboardLayout;
use crate::window::WebWindow;
use anyhow::Result;
use futures::channel::oneshot;
use open_gpui::{
    Action, ActivityGuard, AnyWindowHandle, BackgroundExecutor, ClipboardEntry, ClipboardItem,
    ClipboardReadError, ClipboardString, DisplayId, DummyKeyboardMapper, ForegroundExecutor, Image,
    ImageFormat, Keymap, Menu, MenuItem, NativeAppNotificationHandler, PathPromptOptions, Platform,
    PlatformDisplay, PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem,
    PlatformWindow, PlatformWindowActivationSupport, PlatformWindowCapabilities,
    PlatformWindowCreationCapabilities, PlatformWindowMutationCapabilities, Task, ThermalState,
    WindowAppearance, WindowCreationSupport, WindowInitialPresentationOrder, WindowKind,
    WindowMutationSupport, WindowParams,
};
use open_gpui_wgpu::{
    PreparedWebGraphics, WebBackendPreference, WebGpuContextOptions, WgpuContext, wgpu,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use wasm_bindgen::prelude::*;

static BUNDLED_FONTS: &[&[u8]] = damascene_fonts::DEFAULT_FONTS;

pub struct WebPlatform {
    browser_window: web_sys::Window,
    platform_info: open_gpui::PlatformInfo,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    dispatcher_mode: WebDispatcherMode,
    text_system: Arc<dyn PlatformTextSystem>,
    active_window: Rc<RefCell<Option<AnyWindowHandle>>>,
    active_display: Rc<dyn PlatformDisplay>,
    callbacks: RefCell<WebPlatformCallbacks>,
    webgpu_options: WebGpuContextOptions,
    wgpu_context: Rc<RefCell<Option<WgpuContext>>>,
    prepared_window: Rc<RefCell<Option<PreparedWebWindow>>>,
    window_lifecycle: Rc<Cell<WebWindowLifecycle>>,
    cursor_visible: Rc<Cell<bool>>,
    last_cursor_css: Rc<Cell<&'static str>>,
    _cursor_restore_listeners: Vec<EventListenerHandle>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WebPlatformOptions {
    pub allow_multi_threading: bool,
    pub backend_preference: WebBackendPreference,
    pub force_fallback_adapter: bool,
}

struct PreparedWebWindow {
    canvas: web_sys::HtmlCanvasElement,
    surface: wgpu::Surface<'static>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WebWindowLifecycle {
    Available,
    Open,
    Closed,
    Unavailable,
}

#[derive(Debug)]
pub enum WebWindowError {
    AlreadyOpen,
    ReopeningUnsupported,
    UnsupportedWindowKind(&'static str),
    GraphicsInitializationPending,
    GraphicsUnavailable,
}

impl std::fmt::Display for WebWindowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::AlreadyOpen => "GPUI web supports only one top-level window",
            Self::ReopeningUnsupported => "reopening the GPUI web top-level window is unsupported",
            Self::GraphicsInitializationPending => "browser graphics initialization is pending",
            Self::GraphicsUnavailable => "browser graphics initialization failed",
            Self::UnsupportedWindowKind(kind) => {
                return write!(
                    formatter,
                    "GPUI web does not support a separate {kind} window"
                );
            }
        })
    }
}

impl std::error::Error for WebWindowError {}

#[derive(Default)]
struct WebPlatformCallbacks {
    open_urls: Option<Box<dyn FnMut(Vec<String>)>>,
    quit: Option<Box<dyn FnMut() -> bool>>,
    reopen: Option<Box<dyn FnMut()>>,
    app_menu_action: Option<Box<dyn FnMut(&dyn Action)>>,
    will_open_app_menu: Option<Box<dyn FnMut()>>,
    validate_app_menu_command: Option<Box<dyn FnMut(&dyn Action) -> bool>>,
    keyboard_layout_change: Option<Box<dyn FnMut()>>,
    thermal_state_change: Option<NativeAppNotificationHandler>,
}

impl WebPlatform {
    pub fn new(allow_multi_threading: bool) -> Self {
        Self::new_with_options(WebPlatformOptions {
            allow_multi_threading,
            ..Default::default()
        })
    }

    pub fn new_with_options(options: WebPlatformOptions) -> Self {
        let browser_window =
            web_sys::window().expect("must be running in a browser window context");
        let platform_info = crate::events::platform_info(&browser_window);
        let dispatcher = Arc::new(WebDispatcher::new(
            browser_window.clone(),
            options.allow_multi_threading,
        ));
        let dispatcher_mode = dispatcher.mode();
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher);
        let text_system = Arc::new(open_gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            "Inter",
        ));
        if BUNDLED_FONTS.is_empty() {
            log::warn!("WebPlatform started without bundled fonts");
        } else {
            let fonts = BUNDLED_FONTS
                .iter()
                .map(|bytes| Cow::Borrowed(*bytes))
                .collect();
            if let Err(error) = text_system.add_fonts(fonts) {
                log::error!("failed to load bundled fonts: {error:#}");
            }
        }
        let text_system: Arc<dyn PlatformTextSystem> = text_system;
        let active_display: Rc<dyn PlatformDisplay> =
            Rc::new(WebDisplay::new(browser_window.clone()));

        let cursor_visible = Rc::new(Cell::new(true));
        let last_cursor_css = Rc::new(Cell::new("default"));
        let cursor_restore_listeners = cursor_restore_listeners(
            &browser_window,
            cursor_visible.clone(),
            last_cursor_css.clone(),
        );

        Self {
            browser_window,
            platform_info,
            background_executor,
            foreground_executor,
            dispatcher_mode,
            text_system,
            active_window: Rc::new(RefCell::new(None)),
            active_display,
            callbacks: RefCell::new(WebPlatformCallbacks::default()),
            webgpu_options: WebGpuContextOptions {
                backend_preference: options.backend_preference,
                force_fallback_adapter: options.force_fallback_adapter,
            },
            wgpu_context: Rc::new(RefCell::new(None)),
            prepared_window: Rc::new(RefCell::new(None)),
            window_lifecycle: Rc::new(Cell::new(WebWindowLifecycle::Available)),
            cursor_visible,
            last_cursor_css,
            _cursor_restore_listeners: cursor_restore_listeners,
        }
    }

    pub fn dispatcher_mode(&self) -> WebDispatcherMode {
        self.dispatcher_mode
    }

    /// Returns an HTTP client that runs browser Fetch operations on this platform's main thread.
    pub fn fetch_http_client(&self) -> FetchHttpClient {
        FetchHttpClient::new(self.foreground_executor.to_send())
    }

    /// Returns a browser Fetch HTTP client with the given reported user agent.
    pub fn fetch_http_client_with_user_agent(
        &self,
        user_agent: &str,
    ) -> anyhow::Result<FetchHttpClient> {
        FetchHttpClient::with_user_agent(self.foreground_executor.to_send(), user_agent)
    }

    /// Returns the backend selected by successful browser graphics initialization.
    ///
    /// This is unavailable before initialization and after initialization failure.
    pub fn graphics_backend(&self) -> Option<open_gpui_wgpu::WgpuBackend> {
        self.wgpu_context
            .borrow()
            .as_ref()
            .map(WgpuContext::backend)
    }
}

async fn initialize_graphics(
    browser_window: &web_sys::Window,
    options: WebGpuContextOptions,
) -> anyhow::Result<(web_sys::HtmlCanvasElement, PreparedWebGraphics)> {
    match options.backend_preference {
        WebBackendPreference::Auto => {
            let webgpu_canvas = WebWindow::prepare_canvas(browser_window)?;
            let webgpu_options = WebGpuContextOptions {
                backend_preference: WebBackendPreference::WebGpu,
                ..options
            };
            let webgpu_result = if wgpu::util::is_browser_webgpu_supported().await {
                WgpuContext::new_web_with_options(&webgpu_canvas, webgpu_options).await
            } else {
                Err(anyhow::anyhow!(
                    "browser WebGPU probe did not return a usable adapter"
                ))
            };
            match webgpu_result {
                Ok(graphics) => Ok((webgpu_canvas, graphics)),
                Err(webgpu_error) => {
                    let element: &web_sys::Element = webgpu_canvas.as_ref();
                    element.remove();
                    log::warn!("WebGPU initialization failed; trying WebGL2: {webgpu_error:#}");
                    let webgl_canvas =
                        WebWindow::prepare_canvas(browser_window).map_err(|error| {
                            anyhow::anyhow!(
                                "WebGPU failure: {webgpu_error:#}. WebGL2 canvas failure: {error:#}"
                            )
                        })?;
                    let webgl_options = WebGpuContextOptions {
                        backend_preference: WebBackendPreference::WebGl,
                        ..options
                    };
                    match WgpuContext::new_web_with_options(&webgl_canvas, webgl_options).await {
                        Ok(graphics) => Ok((webgl_canvas, graphics)),
                        Err(webgl_error) => {
                            let element: &web_sys::Element = webgl_canvas.as_ref();
                            element.remove();
                            Err(anyhow::anyhow!(
                                "No browser graphics backend could be initialized. \
                                 WebGPU failure: {webgpu_error:#}. WebGL2 failure: {webgl_error:#}"
                            ))
                        }
                    }
                }
            }
        }
        WebBackendPreference::WebGpu | WebBackendPreference::WebGl => {
            let canvas = WebWindow::prepare_canvas(browser_window)?;
            match WgpuContext::new_web_with_options(&canvas, options).await {
                Ok(graphics) => Ok((canvas, graphics)),
                Err(error) => {
                    let element: &web_sys::Element = canvas.as_ref();
                    element.remove();
                    Err(anyhow::anyhow!(
                        "Only {:?} was tried because the application requested it: {error:#}",
                        options.backend_preference
                    ))
                }
            }
        }
    }
}

impl Platform for WebPlatform {
    fn platform_info(&self) -> open_gpui::PlatformInfo {
        self.platform_info
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

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        let wgpu_context = self.wgpu_context.clone();
        let webgpu_options = self.webgpu_options;
        let browser_window = self.browser_window.clone();
        let prepared_window = self.prepared_window.clone();
        let window_lifecycle = self.window_lifecycle.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match initialize_graphics(&browser_window, webgpu_options).await {
                Ok((canvas, PreparedWebGraphics { context, surface })) => {
                    log::info!("Browser graphics initialized with {:?}", context.backend());
                    *wgpu_context.borrow_mut() = Some(context);
                    *prepared_window.borrow_mut() = Some(PreparedWebWindow { canvas, surface });
                    on_finish_launching();
                }
                Err(err) => {
                    window_lifecycle.set(WebWindowLifecycle::Unavailable);
                    log::error!("Failed to initialize browser graphics: {err:#}");
                    if let Some(document) = browser_window.document()
                        && let Some(body) = document.body()
                        && let Ok(message) = document.create_element("p")
                    {
                        message.set_text_content(Some(&format!(
                            "Failed to initialize browser graphics: {err}"
                        )));
                        body.append_child(&message).ok();
                    }
                }
            }
        });
    }

    fn quit(&self) {
        log::warn!("WebPlatform::quit called, but quitting is not supported in the browser .");
    }

    fn restart(&self, _binary_path: Option<PathBuf>, _arguments: Vec<std::ffi::OsString>) {}

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {}

    fn hide_other_apps(&self) {}

    fn unhide_other_apps(&self) {}

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.active_display.clone()]
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.active_display.clone())
    }

    fn on_displays_changed(&self, _callback: Box<dyn FnMut()>) {}

    fn active_window(&self) -> Option<AnyWindowHandle> {
        *self.active_window.borrow()
    }

    fn window_capabilities(
        &self,
        _kind: &WindowKind,
        _display_id: Option<DisplayId>,
    ) -> PlatformWindowCapabilities {
        PlatformWindowCapabilities {
            creation: PlatformWindowCreationCapabilities {
                focus_on_appearing: WindowCreationSupport::Supported,
                transient_for: WindowCreationSupport::Unsupported,
                provisional_presentation: WindowCreationSupport::Unsupported,
                initial_presentation_order: WindowInitialPresentationOrder::AfterVisibility,
            },
            mutations: PlatformWindowMutationCapabilities {
                pointer_input: WindowMutationSupport::CreationOnly,
                activation_policy: WindowMutationSupport::CreationOnly,
                ..Default::default()
            },
            activation: PlatformWindowActivationSupport::Observed,
        }
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        match &params.kind {
            WindowKind::Normal => {}
            WindowKind::PopUp => return Err(WebWindowError::UnsupportedWindowKind("popup").into()),
            WindowKind::Floating => {
                return Err(WebWindowError::UnsupportedWindowKind("floating").into());
            }
            WindowKind::Dialog => {
                return Err(WebWindowError::UnsupportedWindowKind("dialog").into());
            }
        }
        match self.window_lifecycle.get() {
            WebWindowLifecycle::Available => {}
            WebWindowLifecycle::Open => return Err(WebWindowError::AlreadyOpen.into()),
            WebWindowLifecycle::Closed => return Err(WebWindowError::ReopeningUnsupported.into()),
            WebWindowLifecycle::Unavailable => {
                return Err(WebWindowError::GraphicsUnavailable.into());
            }
        }
        let context_ref = self.wgpu_context.borrow();
        let context = context_ref
            .as_ref()
            .ok_or(WebWindowError::GraphicsInitializationPending)?;
        let prepared = self
            .prepared_window
            .borrow_mut()
            .take()
            .ok_or(WebWindowError::GraphicsInitializationPending)?;
        let canvas_for_cleanup = prepared.canvas.clone();

        let result = WebWindow::new(
            handle,
            params,
            context,
            prepared.canvas,
            prepared.surface,
            self.window_lifecycle.clone(),
            self.browser_window.clone(),
            self.cursor_visible.clone(),
            self.last_cursor_css.clone(),
            Rc::downgrade(&self.active_window),
        );
        match result {
            Ok(window) => {
                self.window_lifecycle.set(WebWindowLifecycle::Open);
                Ok(Box::new(window))
            }
            Err(error) => {
                let element: &web_sys::Element = canvas_for_cleanup.as_ref();
                element.remove();
                self.window_lifecycle.set(WebWindowLifecycle::Unavailable);
                Err(error)
            }
        }
    }

    fn window_appearance(&self) -> WindowAppearance {
        let Ok(Some(media_query)) = self
            .browser_window
            .match_media("(prefers-color-scheme: dark)")
        else {
            return WindowAppearance::Light;
        };
        if media_query.matches() {
            WindowAppearance::Dark
        } else {
            WindowAppearance::Light
        }
    }

    fn open_url(&self, url: &str) {
        if let Err(error) = self.browser_window.open_with_url(url) {
            log::warn!("Failed to open URL '{url}': {error:?}");
        }
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.callbacks.borrow_mut().open_urls = Some(callback);
    }

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Err(anyhow::anyhow!(
            "prompt_for_paths is not supported on the web"
        )))
        .ok();
        rx
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (sender, receiver) = oneshot::channel();
        sender
            .send(Err(anyhow::anyhow!(
                "prompt_for_new_path is not supported on the web"
            )))
            .ok();
        receiver
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, _path: &Path) {}

    fn open_with_system(&self, _path: &Path) {}

    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.callbacks.borrow_mut().quit = Some(callback);
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().reopen = Some(callback);
    }

    /// Browsers expose Page Visibility, but no system sleep or wake notification.
    fn on_system_sleep(&self, _callback: NativeAppNotificationHandler) {}

    fn on_system_wake(&self, _callback: NativeAppNotificationHandler) {}

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {}

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {}

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.callbacks.borrow_mut().app_menu_action = Some(callback);
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().will_open_app_menu = Some(callback);
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.callbacks.borrow_mut().validate_app_menu_command = Some(callback);
    }

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, callback: NativeAppNotificationHandler) {
        self.callbacks.borrow_mut().thermal_state_change = Some(callback);
    }

    fn prevent_idle_sleep(&self, reason: &str) -> Task<Result<ActivityGuard>> {
        Task::ready(Err(anyhow::anyhow!(
            "Idle sleep prevention for {reason:?} is not supported in the browser"
        )))
    }

    fn compositor_name(&self) -> &'static str {
        "Web"
    }

    fn app_path(&self) -> Result<PathBuf> {
        Err(anyhow::anyhow!("app_path is not available on the web"))
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        Err(anyhow::anyhow!(
            "path_for_auxiliary_executable is not available on the web"
        ))
    }

    fn hide_cursor_until_mouse_moves(&self) {
        if !self.cursor_visible.replace(false) {
            return;
        }
        set_body_cursor(&self.browser_window, "none");
    }

    fn is_cursor_visible(&self) -> bool {
        self.cursor_visible.get()
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        None
    }

    fn read_from_clipboard_async(&self) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        let navigator = self.browser_window.navigator();
        // `navigator.clipboard` is undefined outside secure contexts; probing
        // first avoids a wasm-bindgen abort from invoking a method on
        // `undefined`.
        let Some(clipboard) = browser_clipboard(&navigator, "read") else {
            return Task::ready(Err(ClipboardReadError::Unavailable));
        };
        // `read()` must be called synchronously so it is still covered by the
        // user activation (e.g. the click on a context-menu item) that
        // triggered this read.
        let read = clipboard.read();
        self.foreground_executor.spawn(async move {
            let items = wasm_bindgen_futures::JsFuture::from(read)
                .await
                .map_err(clipboard_read_rejection_error)?;
            let mut entries = Vec::new();
            let mut saw_unsupported_type = false;
            for item in js_sys::Array::from(&items).iter() {
                let item: web_sys::ClipboardItem = item.unchecked_into();
                for mime_type in item.types().iter() {
                    let Some(mime_type) = mime_type.as_string() else {
                        continue;
                    };
                    // Only fetch blobs for types we can convert; copies from
                    // other web apps routinely carry `text/html` and
                    // `web application/...` custom formats whose `getType`
                    // fetches would be wasted or rejected.
                    if mime_type == "text/plain" {
                        match read_clipboard_item_text(&item, &mime_type).await {
                            Ok(text) if !text.is_empty() => {
                                entries.push(ClipboardEntry::String(ClipboardString::new(text)));
                            }
                            Ok(_) => {}
                            Err(error) => log_clipboard_entry_error(&mime_type, &error),
                        }
                    } else if let Some(format) = ImageFormat::from_mime_type(&mime_type) {
                        match read_clipboard_item_bytes(&item, &mime_type).await {
                            Ok(bytes) => {
                                entries
                                    .push(ClipboardEntry::Image(Image::from_bytes(format, bytes)));
                            }
                            Err(error) => log_clipboard_entry_error(&mime_type, &error),
                        }
                    } else {
                        saw_unsupported_type = true;
                    }
                }
            }
            if !entries.is_empty() {
                Ok(Some(ClipboardItem { entries }))
            } else if saw_unsupported_type {
                Err(ClipboardReadError::UnsupportedContent)
            } else {
                Ok(None)
            }
        })
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        if let Some(text) = item.text()
            && let Some(clipboard) =
                browser_clipboard(&self.browser_window.navigator(), "writeText")
        {
            let write = clipboard.write_text(&text);
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(error) = wasm_bindgen_futures::JsFuture::from(write).await {
                    log::warn!(
                        "failed to write clipboard text: {}",
                        js_error_message(&error)
                    );
                }
            });
        }
    }

    fn write_credentials(&self, _url: &str, _username: &str, _password: &[u8]) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "credential storage is not available on the web"
        )))
    }

    fn read_credentials(&self, _url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "credential storage is not available on the web"
        )))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(WebKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().keyboard_layout_change = Some(callback);
    }
}

struct EventListenerHandle {
    target: web_sys::EventTarget,
    event_name: &'static str,
    closure: Closure<dyn FnMut(JsValue)>,
}

impl Drop for EventListenerHandle {
    fn drop(&mut self) {
        self.target
            .remove_event_listener_with_callback(
                self.event_name,
                self.closure.as_ref().unchecked_ref(),
            )
            .ok();
    }
}

fn browser_clipboard(navigator: &web_sys::Navigator, method: &str) -> Option<web_sys::Clipboard> {
    let clipboard = js_sys::Reflect::get(navigator.as_ref(), &"clipboard".into()).ok()?;
    if clipboard.is_undefined() || clipboard.is_null() {
        return None;
    }
    let method = js_sys::Reflect::get(&clipboard, &method.into()).ok()?;
    method
        .is_function()
        .then(|| clipboard.unchecked_into::<web_sys::Clipboard>())
}

/// Maps a `navigator.clipboard.read()` rejection to a [`ClipboardReadError`].
///
/// Only `NotAllowedError` and `SecurityError` mean the user or browser
/// refused access; other rejections (e.g. `DataError`) indicate content the
/// clipboard could not represent, where "allow clipboard access" guidance
/// would mislead.
fn clipboard_read_rejection_error(error: JsValue) -> ClipboardReadError {
    match js_error_name(&error).as_deref() {
        Some("NotAllowedError") | Some("SecurityError") => {
            ClipboardReadError::Denied(js_error_message(&error))
        }
        _ => ClipboardReadError::UnsupportedContent,
    }
}

async fn read_clipboard_item_blob(
    item: &web_sys::ClipboardItem,
    mime_type: &str,
) -> Result<web_sys::Blob, JsValue> {
    let blob = wasm_bindgen_futures::JsFuture::from(item.get_type(mime_type)).await?;
    Ok(blob.unchecked_into())
}

async fn read_clipboard_item_text(
    item: &web_sys::ClipboardItem,
    mime_type: &str,
) -> Result<String, JsValue> {
    let blob = read_clipboard_item_blob(item, mime_type).await?;
    let text = wasm_bindgen_futures::JsFuture::from(blob.text()).await?;
    text.as_string()
        .ok_or_else(|| JsValue::from_str("blob text is not a string"))
}

async fn read_clipboard_item_bytes(
    item: &web_sys::ClipboardItem,
    mime_type: &str,
) -> Result<Vec<u8>, JsValue> {
    let blob = read_clipboard_item_blob(item, mime_type).await?;
    read_blob_bytes(&blob).await
}

pub(crate) async fn read_blob_bytes(blob: &web_sys::Blob) -> Result<Vec<u8>, JsValue> {
    let buffer = wasm_bindgen_futures::JsFuture::from(blob.array_buffer()).await?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

fn js_error_name(error: &JsValue) -> Option<String> {
    js_sys::Reflect::get(error, &"name".into())
        .ok()
        .and_then(|name| name.as_string())
}

pub(crate) fn js_error_message(error: &JsValue) -> String {
    js_sys::Reflect::get(error, &"message".into())
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "unknown browser error".to_string())
}

fn log_clipboard_entry_error(mime_type: &str, error: &JsValue) {
    log::warn!(
        "failed to read clipboard entry with type {mime_type}: {}",
        js_error_message(error)
    );
}

fn cursor_restore_listeners(
    browser_window: &web_sys::Window,
    cursor_visible: Rc<Cell<bool>>,
    last_cursor_css: Rc<Cell<&'static str>>,
) -> Vec<EventListenerHandle> {
    let mut handles = Vec::new();
    let Some(document) = browser_window.document() else {
        return handles;
    };

    let make_restore_handler = |browser_window: web_sys::Window| {
        let cursor_visible = cursor_visible.clone();
        let last_cursor_css = last_cursor_css.clone();
        Closure::<dyn FnMut(JsValue)>::new(move |_event: JsValue| {
            if !cursor_visible.replace(true) {
                set_body_cursor(&browser_window, last_cursor_css.get());
            }
        })
    };

    let mut add_listener = |target: &web_sys::EventTarget, event_name: &'static str| {
        let closure = make_restore_handler(browser_window.clone());
        target
            .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
            .ok();
        handles.push(EventListenerHandle {
            target: target.clone(),
            event_name,
            closure,
        });
    };

    let document_target: &web_sys::EventTarget = document.as_ref();
    let window_target: &web_sys::EventTarget = browser_window.as_ref();

    add_listener(document_target, "mousemove");
    add_listener(document_target, "mouseenter");
    add_listener(window_target, "blur");
    add_listener(document_target, "visibilitychange");

    handles
}

pub(crate) fn set_body_cursor(browser_window: &web_sys::Window, css_cursor: &str) {
    if let Some(document) = browser_window.document()
        && let Some(body) = document.body()
        && let Err(error) = body.style().set_property("cursor", css_cursor)
    {
        log::warn!("Failed to set cursor style: {error:?}");
    }
}
