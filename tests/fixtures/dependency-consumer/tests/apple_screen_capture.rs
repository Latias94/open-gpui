#![cfg(all(target_os = "macos", feature = "mac-screen-capture"))]

use open_gpui::{
    App, ForegroundExecutor, PlatformPixelBuffer, ScreenCaptureSource, Surface, surface,
};
use open_gpui_macos::MacPlatform;

#[test]
fn ordinary_mac_backend_uses_the_same_retained_buffer_for_capture_and_surface() {
    let _backend_constructor = MacPlatform::new;
    let _read_sources = |app: &App| app.screen_capture_sources();
    let _start_stream = |source: &dyn ScreenCaptureSource, executor: &ForegroundExecutor| {
        let _metadata = source.metadata();
        source.stream(
            executor,
            Box::new(|frame| {
                let pixels: PlatformPixelBuffer = frame.0;
                let _preview: Surface = surface(pixels);
            }),
        )
    };
}
