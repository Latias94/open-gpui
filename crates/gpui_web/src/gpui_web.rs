pub mod canvas_fallback;

pub use canvas_fallback::CanvasFontFallback;

#[cfg(target_family = "wasm")]
mod canvas_text;
#[cfg(target_family = "wasm")]
mod dispatcher;
#[cfg(target_family = "wasm")]
mod display;
#[cfg(target_family = "wasm")]
mod events;
#[cfg(any(target_family = "wasm", test))]
mod frame_loop;
#[cfg(any(target_family = "wasm", test))]
mod glyph_cache;
#[cfg(target_family = "wasm")]
mod http_client;
#[cfg(target_family = "wasm")]
mod ime_mirror;
#[cfg(any(target_family = "wasm", test))]
mod interaction;
#[cfg(target_family = "wasm")]
mod keyboard;
#[cfg(target_family = "wasm")]
mod logging;
#[cfg(target_family = "wasm")]
mod platform;
#[cfg(any(target_family = "wasm", test))]
mod platform_info;
#[cfg(any(target_family = "wasm", test))]
mod pointer_session;
#[cfg(any(target_family = "wasm", test))]
mod run_replacements;
#[cfg(any(target_family = "wasm", test))]
mod text_edit;
#[cfg(target_family = "wasm")]
mod text_system;
#[cfg(any(target_family = "wasm", test))]
mod touch_session;
#[cfg(target_family = "wasm")]
mod viewport;
#[cfg(target_family = "wasm")]
mod window;

#[cfg(target_family = "wasm")]
pub use dispatcher::{WebDispatcher, WebDispatcherMode, WebDispatcherSingleThreadedReason};
#[cfg(target_family = "wasm")]
pub use display::WebDisplay;
#[cfg(target_family = "wasm")]
pub use http_client::{FetchCredentials, FetchHttpClient};
#[cfg(target_family = "wasm")]
pub use keyboard::WebKeyboardLayout;
#[cfg(target_family = "wasm")]
pub use logging::init_logging;
#[cfg(target_family = "wasm")]
pub use open_gpui_wgpu::WebBackendPreference;
#[cfg(target_family = "wasm")]
pub use platform::{WebPlatform, WebPlatformOptions};
#[cfg(target_family = "wasm")]
pub use window::WebWindow;
