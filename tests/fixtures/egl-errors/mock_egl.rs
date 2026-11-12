#![allow(non_snake_case)]

use khronos_egl::*;
use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering};

static RESULT: AtomicU32 = AtomicU32::new(FALSE);
static ERROR: AtomicI32 = AtomicI32::new(SUCCESS);
static ERROR_READS: AtomicUsize = AtomicUsize::new(0);
static SURFACE: u8 = 0;

#[unsafe(no_mangle)]
pub extern "system" fn mockSetResult(result: Boolean, error: Int) {
    RESULT.store(result, Ordering::Relaxed);
    ERROR.store(error, Ordering::Relaxed);
    ERROR_READS.store(0, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "system" fn mockErrorReads() -> usize {
    ERROR_READS.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "system" fn eglGetError() -> Int {
    ERROR_READS.fetch_add(1, Ordering::Relaxed);
    ERROR.swap(SUCCESS, Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn eglInitialize(
    _display: EGLDisplay,
    major: *mut Int,
    minor: *mut Int,
) -> Boolean {
    let result = RESULT.load(Ordering::Relaxed);
    if result == TRUE {
        unsafe {
            major.write(1);
            minor.write(5);
        }
    }
    result
}

fn surface() -> EGLSurface {
    if RESULT.load(Ordering::Relaxed) == TRUE {
        std::ptr::addr_of!(SURFACE).cast_mut().cast()
    } else {
        NO_SURFACE
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn eglCreateWindowSurface(
    _display: EGLDisplay,
    _config: EGLConfig,
    _window: NativeWindowType,
    _attributes: *const Int,
) -> EGLSurface {
    surface()
}

#[unsafe(no_mangle)]
pub extern "system" fn eglCreatePlatformWindowSurface(
    _display: EGLDisplay,
    _config: EGLConfig,
    _window: *mut c_void,
    _attributes: *const Attrib,
) -> EGLSurface {
    surface()
}

macro_rules! unexpected_calls {
    ($(fn $name:ident($($argument:ident: $argument_type:ty),*) -> $result:ty;)*) => {
        $(
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name($($argument: $argument_type),*) -> $result {
                let _ = ($($argument),*);
                std::process::abort()
            }
        )*
    };
}

unexpected_calls! {
    fn eglChooseConfig(display: EGLDisplay, attributes: *const Int, configs: *mut EGLConfig, capacity: Int, count: *mut Int) -> Boolean;
    fn eglCopyBuffers(display: EGLDisplay, surface: EGLSurface, target: NativePixmapType) -> Boolean;
    fn eglCreateContext(display: EGLDisplay, config: EGLConfig, shared: EGLContext, attributes: *const Int) -> EGLContext;
    fn eglCreatePbufferSurface(display: EGLDisplay, config: EGLConfig, attributes: *const Int) -> EGLSurface;
    fn eglCreatePixmapSurface(display: EGLDisplay, config: EGLConfig, pixmap: NativePixmapType, attributes: *const Int) -> EGLSurface;
    fn eglDestroyContext(display: EGLDisplay, context: EGLContext) -> Boolean;
    fn eglDestroySurface(display: EGLDisplay, surface: EGLSurface) -> Boolean;
    fn eglGetConfigAttrib(display: EGLDisplay, config: EGLConfig, attribute: Int, value: *mut Int) -> Boolean;
    fn eglGetConfigs(display: EGLDisplay, configs: *mut EGLConfig, capacity: Int, count: *mut Int) -> Boolean;
    fn eglGetCurrentDisplay() -> EGLDisplay;
    fn eglGetCurrentSurface(which: Int) -> EGLSurface;
    fn eglGetDisplay(native: NativeDisplayType) -> EGLDisplay;
    fn eglGetProcAddress(name: *const c_char) -> extern "system" fn();
    fn eglMakeCurrent(display: EGLDisplay, draw: EGLSurface, read: EGLSurface, context: EGLContext) -> Boolean;
    fn eglQueryContext(display: EGLDisplay, context: EGLContext, attribute: Int, value: *mut Int) -> Boolean;
    fn eglQueryString(display: EGLDisplay, name: Int) -> *const c_char;
    fn eglQuerySurface(display: EGLDisplay, surface: EGLSurface, attribute: Int, value: *mut Int) -> Boolean;
    fn eglSwapBuffers(display: EGLDisplay, surface: EGLSurface) -> Boolean;
    fn eglTerminate(display: EGLDisplay) -> Boolean;
    fn eglWaitGL() -> Boolean;
    fn eglWaitNative(engine: Int) -> Boolean;
    fn eglBindTexImage(display: EGLDisplay, surface: EGLSurface, buffer: Int) -> Boolean;
    fn eglReleaseTexImage(display: EGLDisplay, surface: EGLSurface, buffer: Int) -> Boolean;
    fn eglSurfaceAttrib(display: EGLDisplay, surface: EGLSurface, attribute: Int, value: Int) -> Boolean;
    fn eglSwapInterval(display: EGLDisplay, interval: Int) -> Boolean;
    fn eglBindAPI(api: Enum) -> Boolean;
    fn eglQueryAPI() -> Enum;
    fn eglCreatePbufferFromClientBuffer(display: EGLDisplay, kind: Enum, buffer: EGLClientBuffer, config: EGLConfig, attributes: *const Int) -> EGLSurface;
    fn eglReleaseThread() -> Boolean;
    fn eglWaitClient() -> Boolean;
    fn eglGetCurrentContext() -> EGLContext;
    fn eglCreateSync(display: EGLDisplay, kind: Enum, attributes: *const Attrib) -> EGLSync;
    fn eglDestroySync(display: EGLDisplay, sync: EGLSync) -> Boolean;
    fn eglClientWaitSync(display: EGLDisplay, sync: EGLSync, flags: Int, timeout: Time) -> Int;
    fn eglGetSyncAttrib(display: EGLDisplay, sync: EGLSync, attribute: Int, value: *mut Attrib) -> Boolean;
    fn eglCreateImage(display: EGLDisplay, context: EGLContext, target: Enum, buffer: EGLClientBuffer, attributes: *const Attrib) -> EGLImage;
    fn eglDestroyImage(display: EGLDisplay, image: EGLImage) -> Boolean;
    fn eglGetPlatformDisplay(platform: Enum, native: *mut c_void, attributes: *const Attrib) -> EGLDisplay;
    fn eglCreatePlatformPixmapSurface(display: EGLDisplay, config: EGLConfig, pixmap: *mut c_void, attributes: *const Attrib) -> EGLSurface;
    fn eglWaitSync(display: EGLDisplay, sync: EGLSync, flags: Int) -> Boolean;
}
