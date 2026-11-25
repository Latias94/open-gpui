use objc2::{class, msg_send, rc::Retained, runtime::AnyObject, runtime::Sel};
use objc2_app_kit::{NSView, NSViewLayerContentsRedrawPolicy};
use objc2_quartz_core::{CAAutoresizingMask, CAMetalLayer};
use raw_window_handle as rwh;
use std::{
    ffi::CStr,
    ffi::c_void,
    ptr::{self, NonNull},
};

pub(super) const LAYER_IVAR: &CStr = c"gpuiWgpuSurfaceLayer";

#[derive(Clone, Copy, Debug)]
pub(super) struct AppKitSurfaceHandle(NonNull<c_void>);

impl AppKitSurfaceHandle {
    pub(super) fn new(view: &NSView) -> Self {
        Self(NonNull::from(view).cast())
    }
}

// SAFETY: The handle carries only the view address. WGPU may keep a copy for its display
// handle, which needs no NSView access. Only AppKit's main thread can obtain a window handle,
// and the native owner keeps the view alive for every surface creation and recovery.
unsafe impl Send for AppKitSurfaceHandle {}
unsafe impl Sync for AppKitSurfaceHandle {}

impl rwh::HasWindowHandle for AppKitSurfaceHandle {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        if crate::macos::main_thread_marker().is_none() {
            return Err(rwh::HandleError::Unavailable);
        }
        Ok(unsafe {
            rwh::WindowHandle::borrow_raw(rwh::RawWindowHandle::AppKit(
                rwh::AppKitWindowHandle::new(self.0),
            ))
        })
    }
}

impl rwh::HasDisplayHandle for AppKitSurfaceHandle {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::appkit())
    }
}

pub(super) fn install_layer(view: &NSView, scale_factor: f32) {
    let layer = CAMetalLayer::new();
    layer.setContentsScale(f64::from(scale_factor));
    layer.setNeedsDisplayOnBoundsChange(true);
    layer.setAutoresizingMask(
        CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable,
    );
    unsafe { layer_slot(view).write(Retained::into_raw(layer)) };
    view.setWantsLayer(true);
    view.setLayerContentsRedrawPolicy(NSViewLayerContentsRedrawPolicy::DuringViewResize);
}

pub(super) fn layer(view: &AnyObject) -> Option<Retained<CAMetalLayer>> {
    unsafe { Retained::retain(*layer_slot(view)) }
}

pub(super) fn detach_layer(view: &NSView) {
    if let Some(_retired_layer) = take_layer(view) {
        view.setWantsLayer(false);
        view.setLayer(None);
    }
}

pub(super) fn take_layer(view: &AnyObject) -> Option<Retained<CAMetalLayer>> {
    unsafe { Retained::from_raw(layer_slot(view).replace(ptr::null_mut())) }
}

pub(super) extern "C" fn make_backing_layer(view: &AnyObject, _: Sel) -> *mut AnyObject {
    unsafe {
        let layer = *layer_slot(view);
        if layer.is_null() {
            msg_send![super(view, class!(NSView)), makeBackingLayer]
        } else {
            layer.cast()
        }
    }
}

unsafe fn layer_slot(view: &AnyObject) -> *mut *mut CAMetalLayer {
    unsafe {
        view.class()
            .instance_variable(LAYER_IVAR)
            .expect("GPUI view surface layer ivar")
            .load_ptr(view)
    }
}
