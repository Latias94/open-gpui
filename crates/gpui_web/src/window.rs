use crate::display::WebDisplay;
use crate::platform::set_body_cursor;
use crate::{
    events::{WebEventListeners, is_mac_platform},
    pointer_session::{ClickState, WebPointerCaptureState},
};
use std::sync::Arc;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use open_gpui::{
    AnyWindowHandle, Bounds, Capslock, CursorStyle, Decorations, DevicePixels, GpuSpecs, Modifiers,
    Pixels, PlatformAtlas, PlatformDisplay, PlatformInputCallback, PlatformInputCallbackSlot,
    PlatformInputHandler, PlatformInputHandlerSlot, PlatformPresentationShutdownOutcome,
    PlatformWindow, PlatformWindowActiveStatusObservation, PlatformWindowCommand,
    PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome, PlatformWindowPresentOutcome,
    Point, PointerCancelReason, PreparedPlatformPresentationShutdown, PromptButton, PromptLevel,
    RequestFrameOptions, Scene, Size, WindowActivationPolicy, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowControls,
    WindowCoordinateSpace, WindowCreationFacts, WindowDecorations, WindowParams,
    WindowPlatformFacts, WindowPresentationShutdownTicket, px,
};
use open_gpui_wgpu::{WgpuContext, WgpuRenderer, WgpuSurfaceConfig, WgpuSurfaceShutdownProgress};
use wasm_bindgen::prelude::*;

#[derive(Default)]
pub(crate) struct WebWindowCallbacks {
    pub(crate) request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    pub(crate) input: PlatformInputCallbackSlot,
    pub(crate) active_status_change: Option<Box<dyn FnMut(PlatformWindowActiveStatusObservation)>>,
    pub(crate) hover_status_change: Option<Box<dyn FnMut(bool)>>,
    pub(crate) resize: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    pub(crate) moved: Option<Box<dyn FnMut()>>,
    pub(crate) should_close: Option<Box<dyn FnMut() -> bool>>,
    pub(crate) close: Option<Box<dyn FnOnce()>>,
    pub(crate) appearance_changed: Option<Box<dyn FnMut()>>,
    pub(crate) hit_test_window_control: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
}

pub(crate) struct WebWindowMutableState {
    pub(crate) renderer: WgpuRenderer,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) scale_factor: f32,
    pub(crate) max_texture_dimension: u32,
    pub(crate) title: String,
    pub(crate) input_handler: PlatformInputHandlerSlot,
    pub(crate) is_fullscreen: bool,
    pub(crate) is_active: bool,
    pub(crate) is_hovered: bool,
    pub(crate) mouse_position: Point<Pixels>,
    pub(crate) modifiers: Modifiers,
    pub(crate) capslock: Capslock,
}

pub(crate) struct WebWindowInner {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) browser_window: web_sys::Window,
    pub(crate) root_element: web_sys::HtmlElement,
    pub(crate) canvas: web_sys::HtmlCanvasElement,
    pub(crate) input_element: web_sys::HtmlInputElement,
    pub(crate) has_device_pixel_support: bool,
    pub(crate) is_mac: bool,
    pub(crate) state: RefCell<WebWindowMutableState>,
    pub(crate) callbacks: RefCell<WebWindowCallbacks>,
    pub(crate) click_state: RefCell<ClickState>,
    pub(crate) pointer_capture: Cell<WebPointerCaptureState>,
    pub(crate) last_physical_size: Cell<(u32, u32)>,
    pub(crate) notify_scale: Cell<bool>,
    pub(crate) is_composing: Cell<bool>,
    pub(crate) cursor_visible: Rc<Cell<bool>>,
    pub(crate) last_cursor_css: Rc<Cell<&'static str>>,
    active_window: Weak<RefCell<Option<AnyWindowHandle>>>,
    creation_facts: WindowCreationFacts,
    activation_policy: WindowActivationPolicy,
    accepts_pointer_input: bool,
    is_mapped: Cell<bool>,
    initial_presentation_completed: Cell<bool>,
    raf_running: Cell<bool>,
    raf_id: Cell<Option<i32>>,
    raf_function: RefCell<Option<js_sys::Function>>,
    mql_handle: RefCell<Option<MqlHandle>>,
    pending_physical_size: Cell<Option<(u32, u32)>>,
}

pub struct WebWindow {
    inner: Rc<WebWindowInner>,
    display: Rc<dyn PlatformDisplay>,
    #[allow(dead_code)]
    handle: AnyWindowHandle,
    _raf_closure: Closure<dyn FnMut()>,
    _resize_observer: Option<web_sys::ResizeObserver>,
    _resize_observer_closure: Closure<dyn FnMut(js_sys::Array)>,
    event_listeners: Option<WebEventListeners>,
}

impl WebWindow {
    pub fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        context: &WgpuContext,
        browser_window: web_sys::Window,
        cursor_visible: Rc<Cell<bool>>,
        last_cursor_css: Rc<Cell<&'static str>>,
        active_window: Weak<RefCell<Option<AnyWindowHandle>>>,
    ) -> anyhow::Result<Self> {
        let document = browser_window
            .document()
            .ok_or_else(|| anyhow::anyhow!("No `document` found on window"))?;

        let root_element: web_sys::HtmlElement = document
            .create_element("div")
            .map_err(|e| anyhow::anyhow!("Failed to create GPUI root element: {e:?}"))?
            .dyn_into()
            .map_err(|e| anyhow::anyhow!("Created GPUI root is not an HTML element: {e:?}"))?;
        let root_style = root_element.style();
        root_style
            .set_property("position", "fixed")
            .map_err(|e| anyhow::anyhow!("Failed to set root position style: {e:?}"))?;
        root_style
            .set_property("inset", "0")
            .map_err(|e| anyhow::anyhow!("Failed to set root inset style: {e:?}"))?;
        root_style
            .set_property("overflow", "hidden")
            .map_err(|e| anyhow::anyhow!("Failed to set root overflow style: {e:?}"))?;
        if !params.accepts_pointer_input {
            root_style
                .set_property("pointer-events", "none")
                .map_err(|e| anyhow::anyhow!("Failed to disable root pointer input: {e:?}"))?;
        }

        let canvas: web_sys::HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|e| anyhow::anyhow!("Failed to create canvas element: {e:?}"))?
            .dyn_into()
            .map_err(|e| anyhow::anyhow!("Created element is not a canvas: {e:?}"))?;

        let dpr = browser_window.device_pixel_ratio() as f32;
        let max_texture_dimension = context.device.limits().max_texture_dimension_2d;
        let has_device_pixel_support = check_device_pixel_support();

        canvas.set_tab_index(-1);

        let style = canvas.style();
        style
            .set_property("width", "100%")
            .map_err(|e| anyhow::anyhow!("Failed to set canvas width style: {e:?}"))?;
        style
            .set_property("height", "100%")
            .map_err(|e| anyhow::anyhow!("Failed to set canvas height style: {e:?}"))?;
        style
            .set_property("display", "block")
            .map_err(|e| anyhow::anyhow!("Failed to set canvas display style: {e:?}"))?;
        style
            .set_property("outline", "none")
            .map_err(|e| anyhow::anyhow!("Failed to set canvas outline style: {e:?}"))?;
        style
            .set_property("touch-action", "none")
            .map_err(|e| anyhow::anyhow!("Failed to set touch-action style: {e:?}"))?;

        root_element
            .append_child(&canvas)
            .map_err(|e| anyhow::anyhow!("Failed to append canvas to GPUI root: {e:?}"))?;

        let input_element: web_sys::HtmlInputElement = document
            .create_element("input")
            .map_err(|e| anyhow::anyhow!("Failed to create input element: {e:?}"))?
            .dyn_into()
            .map_err(|e| anyhow::anyhow!("Created element is not an input: {e:?}"))?;
        let input_style = input_element.style();
        input_element.set_tab_index(-1);
        input_style.set_property("position", "fixed").ok();
        input_style.set_property("top", "0").ok();
        input_style.set_property("left", "0").ok();
        input_style.set_property("width", "1px").ok();
        input_style.set_property("height", "1px").ok();
        input_style.set_property("opacity", "0").ok();
        root_element
            .append_child(&input_element)
            .map_err(|e| anyhow::anyhow!("Failed to append input to GPUI root: {e:?}"))?;

        let device_size = Size {
            width: DevicePixels(0),
            height: DevicePixels(0),
        };

        let renderer_config = WgpuSurfaceConfig {
            size: device_size,
            transparent: false,
            preferred_present_mode: None,
        };

        let renderer = WgpuRenderer::new_from_canvas(context, &canvas, renderer_config)?;

        let display: Rc<dyn PlatformDisplay> = Rc::new(WebDisplay::new(browser_window.clone()));

        let initial_bounds = Bounds {
            origin: Point::default(),
            size: Size::default(),
        };

        let mutable_state = WebWindowMutableState {
            renderer,
            bounds: initial_bounds,
            scale_factor: dpr,
            max_texture_dimension,
            title: String::new(),
            input_handler: PlatformInputHandlerSlot::default(),
            is_fullscreen: false,
            is_active: false,
            is_hovered: false,
            mouse_position: Point::default(),
            modifiers: Modifiers::default(),
            capslock: Capslock::default(),
        };

        let is_mac = is_mac_platform(&browser_window);

        let inner = Rc::new(WebWindowInner {
            handle,
            browser_window,
            root_element,
            canvas,
            input_element,
            has_device_pixel_support,
            is_mac,
            state: RefCell::new(mutable_state),
            callbacks: RefCell::new(WebWindowCallbacks::default()),
            click_state: RefCell::new(ClickState::default()),
            pointer_capture: Cell::new(WebPointerCaptureState::default()),
            last_physical_size: Cell::new((0, 0)),
            notify_scale: Cell::new(false),
            is_composing: Cell::new(false),
            cursor_visible,
            last_cursor_css,
            active_window,
            creation_facts: WindowCreationFacts {
                show: params.show,
                focus_on_appearing: params.focus_on_appearing,
                transient_for: None,
            },
            activation_policy: params.activation_policy,
            accepts_pointer_input: params.accepts_pointer_input,
            is_mapped: Cell::new(false),
            initial_presentation_completed: Cell::new(false),
            raf_running: Cell::new(false),
            raf_id: Cell::new(None),
            raf_function: RefCell::new(None),
            mql_handle: RefCell::new(None),
            pending_physical_size: Cell::new(None),
        });

        let raf_closure = inner.create_raf_closure();
        inner.install_raf_function(&raf_closure);

        let resize_observer_closure = Self::create_resize_observer_closure(Rc::clone(&inner));
        let resize_observer =
            web_sys::ResizeObserver::new(resize_observer_closure.as_ref().unchecked_ref()).ok();

        Ok(Self {
            inner,
            display,
            handle,
            _raf_closure: raf_closure,
            _resize_observer: resize_observer,
            _resize_observer_closure: resize_observer_closure,
            event_listeners: None,
        })
    }

    fn create_resize_observer_closure(
        inner: Rc<WebWindowInner>,
    ) -> Closure<dyn FnMut(js_sys::Array)> {
        Closure::new(move |entries: js_sys::Array| {
            let entry: web_sys::ResizeObserverEntry = match entries.get(0).dyn_into().ok() {
                Some(entry) => entry,
                None => return,
            };

            let dpr = inner.browser_window.device_pixel_ratio();
            let dpr_f32 = dpr as f32;

            let (physical_width, physical_height, logical_width, logical_height) =
                if inner.has_device_pixel_support {
                    let size: web_sys::ResizeObserverSize = entry
                        .device_pixel_content_box_size()
                        .get(0)
                        .unchecked_into();
                    let pw = size.inline_size() as u32;
                    let ph = size.block_size() as u32;
                    let lw = pw as f64 / dpr;
                    let lh = ph as f64 / dpr;
                    (pw, ph, lw as f32, lh as f32)
                } else {
                    // Safari fallback: use contentRect (always CSS px).
                    let rect = entry.content_rect();
                    let lw = rect.width() as f32;
                    let lh = rect.height() as f32;
                    let pw = (lw as f64 * dpr).round() as u32;
                    let ph = (lh as f64 * dpr).round() as u32;
                    (pw, ph, lw, lh)
                };

            let scale_changed = inner.notify_scale.replace(false);
            let prev = inner.last_physical_size.get();
            let size_changed = prev != (physical_width, physical_height);

            if !scale_changed && !size_changed {
                return;
            }
            inner
                .last_physical_size
                .set((physical_width, physical_height));

            let new_size = inner.update_observed_size(
                physical_width,
                physical_height,
                logical_width,
                logical_height,
                dpr_f32,
            );
            inner.dispatch_resize(new_size, dpr_f32);
        })
    }
}

impl WebWindowInner {
    fn update_size_from_canvas_rect(&self) {
        let rect = self.canvas.get_bounding_client_rect();
        let dpr = self.browser_window.device_pixel_ratio();
        let logical_width = rect.width().max(0.0) as f32;
        let logical_height = rect.height().max(0.0) as f32;
        let physical_width = (logical_width as f64 * dpr).round() as u32;
        let physical_height = (logical_height as f64 * dpr).round() as u32;

        self.last_physical_size
            .set((physical_width, physical_height));
        self.update_observed_size(
            physical_width,
            physical_height,
            logical_width,
            logical_height,
            dpr as f32,
        );
    }

    fn update_observed_size(
        &self,
        physical_width: u32,
        physical_height: u32,
        logical_width: f32,
        logical_height: f32,
        dpr: f32,
    ) -> Size<Pixels> {
        let new_size = Size {
            width: px(logical_width),
            height: px(logical_height),
        };

        if physical_width == 0 || physical_height == 0 {
            let mut state = self.state.borrow_mut();
            state.bounds.size = Size::default();
            state.scale_factor = dpr;
            self.pending_physical_size.set(None);
            return Size::default();
        }

        let max_texture_dimension = self.state.borrow().max_texture_dimension;
        self.pending_physical_size.set(Some((
            physical_width.min(max_texture_dimension),
            physical_height.min(max_texture_dimension),
        )));

        {
            let mut state = self.state.borrow_mut();
            state.bounds.size = new_size;
            state.scale_factor = dpr;
        }

        new_size
    }

    pub(crate) fn dispatch_resize(&self, size: Size<Pixels>, scale_factor: f32) {
        let mut callback = {
            let mut callbacks = self.callbacks.borrow_mut();
            callbacks.resize.take()
        };

        if let Some(ref mut callback) = callback {
            callback(size, scale_factor);
        }

        if let Some(callback) = callback {
            self.callbacks.borrow_mut().resize = Some(callback);
        }
    }

    pub(crate) fn dispatch_request_frame(&self, options: RequestFrameOptions) {
        let mut callback = {
            let mut callbacks = self.callbacks.borrow_mut();
            callbacks.request_frame.take()
        };

        if let Some(ref mut callback) = callback {
            callback(options);
        }

        if let Some(callback) = callback {
            self.callbacks.borrow_mut().request_frame = Some(callback);
        }
    }

    fn dispatch_active_status_change(&self, observation: PlatformWindowActiveStatusObservation) {
        let mut callback = {
            let mut callbacks = self.callbacks.borrow_mut();
            callbacks.active_status_change.take()
        };

        if let Some(ref mut callback) = callback {
            callback(observation);
        }

        if let Some(callback) = callback {
            self.callbacks.borrow_mut().active_status_change = Some(callback);
        }
    }

    pub(crate) fn sync_dom_activation(&self) {
        let is_active = self.browser_window.document().is_some_and(|document| {
            let is_visible = js_sys::Reflect::get(&document, &"visibilityState".into())
                .ok()
                .and_then(|state| state.as_string())
                .as_deref()
                == Some("visible");
            let document_has_focus = document.has_focus().unwrap_or(false);
            let input_element: &web_sys::Element = self.input_element.as_ref();
            let input_has_focus = document.active_element().as_ref() == Some(input_element);

            is_visible && document_has_focus && input_has_focus
        });

        let changed = {
            let mut state = self.state.borrow_mut();
            if state.is_active == is_active {
                false
            } else {
                state.is_active = is_active;
                true
            }
        };
        if !changed {
            return;
        }

        self.update_active_window(is_active);
        if !is_active {
            self.cleanup_pointer_capture(PointerCancelReason::WindowDeactivated);
        }
        self.dispatch_active_status_change(PlatformWindowActiveStatusObservation::new(
            is_active, is_active,
        ));
    }

    fn update_active_window(&self, is_active: bool) {
        let Some(active_window) = self.active_window.upgrade() else {
            return;
        };
        let mut active_window = active_window.borrow_mut();
        if is_active {
            *active_window = Some(self.handle);
        } else if *active_window == Some(self.handle) {
            *active_window = None;
        }
    }

    fn focus_input(&self) -> bool {
        if !self.is_mapped.get() {
            return false;
        }
        if let Err(error) = self.input_element.focus() {
            log::warn!("failed to focus GPUI web input: {error:?}");
            return false;
        }

        self.sync_dom_activation();
        true
    }

    fn activate(&self) -> bool {
        self.activation_policy.accepts_activation && self.focus_input()
    }

    pub(crate) fn focus_from_pointer(&self) {
        if self.activation_policy.focus_on_click {
            let _ = self.focus_input();
        }
    }

    fn complete_initial_presentation(&self, activate: bool) {
        if self.initial_presentation_completed.replace(true) {
            return;
        }
        if self.creation_facts.show && self.creation_facts.focus_on_appearing && activate {
            let _ = self.activate();
        }
    }

    pub(crate) fn dispatch_hover_status_change(&self, is_hovered: bool) {
        let mut callback = {
            let mut callbacks = self.callbacks.borrow_mut();
            callbacks.hover_status_change.take()
        };

        if let Some(ref mut callback) = callback {
            callback(is_hovered);
        }

        if let Some(callback) = callback {
            self.callbacks.borrow_mut().hover_status_change = Some(callback);
        }
    }

    pub(crate) fn dispatch_appearance_changed(&self) {
        let mut callback = {
            let mut callbacks = self.callbacks.borrow_mut();
            callbacks.appearance_changed.take()
        };

        if let Some(ref mut callback) = callback {
            callback();
        }

        if let Some(callback) = callback {
            self.callbacks.borrow_mut().appearance_changed = Some(callback);
        }
    }

    fn create_raf_closure(self: &Rc<Self>) -> Closure<dyn FnMut()> {
        let this = Rc::downgrade(self);
        let closure = Closure::new(move || {
            let Some(this) = this.upgrade() else {
                return;
            };
            this.raf_id.set(None);
            if !this.raf_running.get() {
                return;
            }

            this.dispatch_request_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: false,
            });

            this.schedule_raf();
        });

        closure
    }

    fn install_raf_function(&self, closure: &Closure<dyn FnMut()>) {
        let function = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
        *self.raf_function.borrow_mut() = Some(function);
    }

    fn start_raf(&self) {
        if self.raf_running.replace(true) {
            return;
        }
        self.schedule_raf();
    }

    fn schedule_raf(&self) {
        let Some(function) = self.raf_function.borrow().as_ref().cloned() else {
            return;
        };
        match self.browser_window.request_animation_frame(&function) {
            Ok(id) => self.raf_id.set(Some(id)),
            Err(error) => {
                self.raf_running.set(false);
                log::error!("failed to schedule requestAnimationFrame: {error:?}");
            }
        }
    }

    fn stop_raf(&self) {
        self.raf_running.set(false);
        if let Some(id) = self.raf_id.take() {
            self.browser_window.cancel_animation_frame(id).ok();
        }
        self.raf_function.borrow_mut().take();
    }

    fn observe_canvas(&self, observer: &web_sys::ResizeObserver) {
        observer.unobserve(&self.canvas);
        if self.has_device_pixel_support {
            let options = web_sys::ResizeObserverOptions::new();
            options.set_box(web_sys::ResizeObserverBoxOptions::DevicePixelContentBox);
            observer.observe_with_options(&self.canvas, &options);
        } else {
            observer.observe(&self.canvas);
        }
    }

    fn watch_dpr_changes(self: &Rc<Self>, observer: &web_sys::ResizeObserver) {
        let current_dpr = self.browser_window.device_pixel_ratio();
        let media_query =
            format!("(resolution: {current_dpr}dppx), (-webkit-device-pixel-ratio: {current_dpr})");
        let Some(mql) = self.browser_window.match_media(&media_query).ok().flatten() else {
            return;
        };

        let this = Rc::clone(self);
        let observer = observer.clone();

        let closure = Closure::<dyn FnMut(JsValue)>::new(move |_event: JsValue| {
            this.notify_scale.set(true);
            this.observe_canvas(&observer);
            this.watch_dpr_changes(&observer);
        });

        mql.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
            .ok();

        *self.mql_handle.borrow_mut() = Some(MqlHandle {
            mql,
            _closure: closure,
        });
    }

    pub(crate) fn with_input_handler<R>(
        &self,
        f: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Option<R> {
        let input_handler_slot = self.state.borrow().input_handler.clone();
        input_handler_slot.with_handler(f)
    }
}

fn current_appearance(browser_window: &web_sys::Window) -> WindowAppearance {
    let is_dark = browser_window
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten()
        .map(|mql| mql.matches())
        .unwrap_or(false);

    if is_dark {
        WindowAppearance::Dark
    } else {
        WindowAppearance::Light
    }
}

struct MqlHandle {
    mql: web_sys::MediaQueryList,
    _closure: Closure<dyn FnMut(JsValue)>,
}

impl Drop for MqlHandle {
    fn drop(&mut self) {
        self.mql
            .remove_event_listener_with_callback("change", self._closure.as_ref().unchecked_ref())
            .ok();
    }
}

// Safari does not support `devicePixelContentBoxSize`, so detect whether it's available.
fn check_device_pixel_support() -> bool {
    let global: JsValue = js_sys::global().into();
    let Ok(constructor) = js_sys::Reflect::get(&global, &"ResizeObserverEntry".into()) else {
        return false;
    };
    let Ok(prototype) = js_sys::Reflect::get(&constructor, &"prototype".into()) else {
        return false;
    };
    let descriptor = js_sys::Object::get_own_property_descriptor(
        &prototype.unchecked_into::<js_sys::Object>(),
        &"devicePixelContentBoxSize".into(),
    );
    !descriptor.is_undefined()
}

impl raw_window_handle::HasWindowHandle for WebWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let canvas_ref: &JsValue = self.inner.canvas.as_ref();
        let obj = std::ptr::NonNull::from(canvas_ref).cast::<std::ffi::c_void>();
        let handle = raw_window_handle::WebCanvasWindowHandle::new(obj);
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl raw_window_handle::HasDisplayHandle for WebWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::web())
    }
}

fn cursor_style_to_css(style: CursorStyle) -> &'static str {
    match style {
        CursorStyle::Arrow => "default",
        CursorStyle::IBeam => "text",
        CursorStyle::Crosshair => "crosshair",
        CursorStyle::ClosedHand => "grabbing",
        CursorStyle::OpenHand => "grab",
        CursorStyle::PointingHand => "pointer",
        CursorStyle::ResizeLeft | CursorStyle::ResizeRight | CursorStyle::ResizeLeftRight => {
            "ew-resize"
        }
        CursorStyle::ResizeUp | CursorStyle::ResizeDown | CursorStyle::ResizeUpDown => "ns-resize",
        CursorStyle::ResizeUpLeftDownRight => "nesw-resize",
        CursorStyle::ResizeUpRightDownLeft => "nwse-resize",
        CursorStyle::ResizeColumn => "col-resize",
        CursorStyle::ResizeRow => "row-resize",
        CursorStyle::IBeamCursorForVerticalLayout => "vertical-text",
        CursorStyle::OperationNotAllowed => "not-allowed",
        CursorStyle::DragLink => "alias",
        CursorStyle::DragCopy => "copy",
        CursorStyle::ContextualMenu => "context-menu",
    }
}

impl Drop for WebWindow {
    fn drop(&mut self) {
        let can_release_surface = self
            .inner
            .state
            .borrow()
            .renderer
            .surface_owner_release_is_safe();
        if !can_release_surface {
            log::error!(
                "Web window dropped before exact presentation shutdown drained submitted GPU work; retaining backend owner"
            );
            self.inner.stop_raf();
            std::mem::forget(self.inner.clone());
            return;
        }

        self.inner
            .pointer_capture
            .set(WebPointerCaptureState::default());
        let input_callback = self.inner.callbacks.borrow().input.clone();
        let input_handler = self.inner.state.borrow().input_handler.clone();
        input_callback.terminate();
        input_handler.terminate();

        self.inner.stop_raf();
        if let Some(observer) = self._resize_observer.as_ref() {
            observer.disconnect();
        }
        self.inner.mql_handle.borrow_mut().take();

        let close_callback = {
            let mut callbacks = self.inner.callbacks.borrow_mut();
            callbacks.request_frame = None;
            callbacks.active_status_change = None;
            callbacks.hover_status_change = None;
            callbacks.resize = None;
            callbacks.moved = None;
            callbacks.should_close = None;
            callbacks.appearance_changed = None;
            callbacks.hit_test_window_control = None;
            callbacks.close.take()
        };
        self.inner.state.borrow_mut().is_active = false;
        self.inner.update_active_window(false);

        if let Some(parent) = self.inner.root_element.parent_node() {
            parent.remove_child(&self.inner.root_element).ok();
        }
        self.inner.is_mapped.set(false);
        self.event_listeners.take();
        if let Some(callback) = close_callback {
            callback();
        }
    }
}

impl PlatformWindow for WebWindow {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let inner = Rc::downgrade(&self.inner);
        PlatformWindowCommandDispatcher::new(move |command| {
            let Some(inner) = inner.upgrade() else {
                return PlatformWindowCommandOutcome::WindowClosed;
            };

            match command {
                PlatformWindowCommand::CompleteInitialPresentation { activate } => {
                    inner.complete_initial_presentation(activate);
                    PlatformWindowCommandOutcome::Accepted
                }
                PlatformWindowCommand::RevealDeferredInitialPresentation { .. } => {
                    PlatformWindowCommandOutcome::Rejected
                }
                PlatformWindowCommand::Activate { .. } => {
                    if inner.activate() {
                        PlatformWindowCommandOutcome::Accepted
                    } else {
                        PlatformWindowCommandOutcome::Rejected
                    }
                }
                PlatformWindowCommand::ShowWindowMenu(_)
                | PlatformWindowCommand::StartWindowMove
                | PlatformWindowCommand::StartWindowResize(_) => {
                    PlatformWindowCommandOutcome::Rejected
                }
            }
        })
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        let inner = Rc::clone(&self.inner);
        PreparedPlatformPresentationShutdown::new(shutdown, move |shutdown| {
            let Ok(mut state) = inner.state.try_borrow_mut() else {
                return PlatformPresentationShutdownOutcome::Rejected;
            };
            if shutdown.snapshot().window_id() != inner.handle.window_id() {
                return PlatformPresentationShutdownOutcome::Rejected;
            }

            match state.renderer.begin_surface_shutdown(shutdown) {
                WgpuSurfaceShutdownProgress::Quiesced => {
                    if state.renderer.is_quiesced_for(shutdown) {
                        PlatformPresentationShutdownOutcome::Quiesced
                    } else {
                        PlatformPresentationShutdownOutcome::Rejected
                    }
                }
                WgpuSurfaceShutdownProgress::Rejected => {
                    PlatformPresentationShutdownOutcome::Rejected
                }
                WgpuSurfaceShutdownProgress::EnteredDraining => {
                    inner.stop_raf();
                    match state.renderer.advance_web_surface_shutdown(shutdown) {
                        WgpuSurfaceShutdownProgress::Quiesced => {
                            PlatformPresentationShutdownOutcome::Quiesced
                        }
                        WgpuSurfaceShutdownProgress::EnteredDraining
                        | WgpuSurfaceShutdownProgress::Draining
                        | WgpuSurfaceShutdownProgress::Rejected => {
                            PlatformPresentationShutdownOutcome::Rejected
                        }
                    }
                }
                WgpuSurfaceShutdownProgress::Draining => {
                    match state.renderer.advance_web_surface_shutdown(shutdown) {
                        WgpuSurfaceShutdownProgress::Quiesced => {
                            PlatformPresentationShutdownOutcome::Quiesced
                        }
                        WgpuSurfaceShutdownProgress::EnteredDraining
                        | WgpuSurfaceShutdownProgress::Draining
                        | WgpuSurfaceShutdownProgress::Rejected => {
                            PlatformPresentationShutdownOutcome::Rejected
                        }
                    }
                }
            }
        })
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.inner.state.borrow().bounds
    }

    fn map_window(&mut self) -> anyhow::Result<()> {
        if !self.inner.creation_facts.show || self.inner.is_mapped.get() {
            return Ok(());
        }

        let document = self
            .inner
            .browser_window
            .document()
            .ok_or_else(|| anyhow::anyhow!("No `document` found while mapping web window"))?;
        let body = document
            .body()
            .ok_or_else(|| anyhow::anyhow!("No `body` found while mapping web window"))?;
        body.append_child(&self.inner.root_element)
            .map_err(|error| anyhow::anyhow!("Failed to map GPUI web root: {error:?}"))?;
        self.inner.is_mapped.set(true);

        self.inner.update_size_from_canvas_rect();
        if let Some(observer) = self._resize_observer.as_ref() {
            self.inner.observe_canvas(observer);
            self.inner.watch_dpr_changes(observer);
        }
        self.event_listeners = Some(self.inner.register_event_listeners());
        self.inner.start_raf();

        Ok(())
    }

    fn is_maximized(&self) -> bool {
        false
    }

    fn accepts_pointer_input(&self) -> bool {
        self.inner.accepts_pointer_input
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        self.inner.creation_facts.clone()
    }

    fn is_visible(&self) -> bool {
        self.inner.is_mapped.get()
    }

    fn platform_facts(&self) -> WindowPlatformFacts {
        let state = self.inner.state.borrow();
        WindowPlatformFacts {
            bounds: state.bounds,
            coordinate_space: WindowCoordinateSpace::WindowLocal,
            physical_geometry: None,
            window_bounds: WindowBounds::Windowed(state.bounds),
            inner_window_bounds: WindowBounds::Windowed(state.bounds),
            content_size: state.bounds.size,
            scale_factor: state.scale_factor,
            display_id: Some(self.display.id()),
            is_minimized: false,
            is_maximized: false,
            is_fullscreen: state.is_fullscreen,
            accepts_pointer_input: self.inner.accepts_pointer_input,
            accepts_activation: self.inner.activation_policy.accepts_activation,
            focus_on_click: self.inner.activation_policy.focus_on_click,
            background_appearance: WindowBackgroundAppearance::Opaque,
            topmost: false,
            taskbar_visible: false,
            is_active: state.is_active,
        }
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.inner.state.borrow().bounds.size
    }

    fn scale_factor(&self) -> f32 {
        self.inner.state.borrow().scale_factor
    }

    fn appearance(&self) -> WindowAppearance {
        current_appearance(&self.inner.browser_window)
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.display.clone())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.inner.state.borrow().mouse_position
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let css_cursor = cursor_style_to_css(style);
        self.inner.last_cursor_css.set(css_cursor);
        let _ = self.inner.canvas.style().set_property("cursor", css_cursor);
        if self.inner.cursor_visible.get() {
            set_body_cursor(&self.inner.browser_window, css_cursor);
        }
    }

    fn modifiers(&self) -> Modifiers {
        self.inner.state.borrow().modifiers
    }

    fn capslock(&self) -> Capslock {
        self.inner.state.borrow().capslock
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        let input_handler_slot = self.inner.state.borrow().input_handler.clone();
        input_handler_slot.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let input_handler_slot = self.inner.state.borrow().input_handler.clone();
        input_handler_slot.take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<futures::channel::oneshot::Receiver<usize>> {
        None
    }

    fn is_active(&self) -> bool {
        self.inner.state.borrow().is_active
    }

    fn is_hovered(&self) -> bool {
        self.inner.state.borrow().is_hovered
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, title: &str) {
        self.inner.state.borrow_mut().title = title.to_owned();
        if let Some(document) = self.inner.browser_window.document() {
            document.set_title(title);
        }
    }

    fn set_background_appearance(&self, _background: WindowBackgroundAppearance) {}

    fn is_fullscreen(&self) -> bool {
        self.inner.state.borrow().is_fullscreen
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.inner.callbacks.borrow_mut().request_frame = Some(callback);
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        let input_callback = self.inner.callbacks.borrow().input.clone();
        input_callback.set(callback);
    }

    fn on_active_status_change(
        &self,
        callback: Box<dyn FnMut(PlatformWindowActiveStatusObservation)>,
    ) {
        self.inner.callbacks.borrow_mut().active_status_change = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.inner.callbacks.borrow_mut().hover_status_change = Some(callback);
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.inner.callbacks.borrow_mut().resize = Some(callback);
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.inner.callbacks.borrow_mut().moved = Some(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.inner.callbacks.borrow_mut().should_close = Some(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.inner.callbacks.borrow_mut().close = Some(callback);
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.inner.callbacks.borrow_mut().hit_test_window_control = Some(callback);
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.callbacks.borrow_mut().appearance_changed = Some(callback);
    }

    fn draw(&self, scene: &Scene) -> PlatformWindowPresentOutcome {
        if !self.inner.is_mapped.get() {
            return PlatformWindowPresentOutcome::Deferred;
        }

        if self
            .inner
            .state
            .borrow()
            .renderer
            .presentation_shutdown_active()
        {
            return PlatformWindowPresentOutcome::Deferred;
        }

        if let Some((width, height)) = self.inner.pending_physical_size.take() {
            let mut state = self.inner.state.borrow_mut();
            if state.renderer.presentation_shutdown_active() {
                self.inner.pending_physical_size.set(Some((width, height)));
                return PlatformWindowPresentOutcome::Deferred;
            }

            if self.inner.canvas.width() != width || self.inner.canvas.height() != height {
                self.inner.canvas.set_width(width);
                self.inner.canvas.set_height(height);
            }

            state.renderer.update_drawable_size(Size {
                width: DevicePixels(width as i32),
                height: DevicePixels(height as i32),
            });
        }

        let mut state = self.inner.state.borrow_mut();
        if state.renderer.presentation_shutdown_active() {
            return PlatformWindowPresentOutcome::Deferred;
        }
        if state.renderer.device_lost() {
            return PlatformWindowPresentOutcome::Rejected;
        }
        state.renderer.draw(scene)
    }

    fn completed_frame(&self) {
        // On web, presentation happens automatically via wgpu surface present
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.inner.state.borrow().renderer.sprite_atlas().clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        self.inner
            .state
            .borrow()
            .renderer
            .supports_dual_source_blending()
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        Some(self.inner.state.borrow().renderer.gpu_specs())
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {}

    fn request_decorations(&self, _decorations: WindowDecorations) {}

    fn window_decorations(&self) -> Decorations {
        Decorations::Server
    }

    fn set_app_id(&mut self, _app_id: &str) {}

    fn window_controls(&self) -> WindowControls {
        WindowControls {
            fullscreen: true,
            maximize: false,
            minimize: false,
            window_menu: false,
        }
    }

    fn set_client_inset(&self, _inset: Pixels) {}
}
