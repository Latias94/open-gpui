#[cfg(target_family = "wasm")]
mod dispatcher;
#[cfg(target_family = "wasm")]
mod display;
#[cfg(target_family = "wasm")]
mod events;
#[cfg(target_family = "wasm")]
mod http_client;
#[cfg(target_family = "wasm")]
mod keyboard;
#[cfg(target_family = "wasm")]
mod logging;
#[cfg(target_family = "wasm")]
mod platform;
#[cfg(any(target_family = "wasm", test))]
mod pointer_session;
#[cfg(target_family = "wasm")]
mod window;

#[cfg(target_family = "wasm")]
pub use dispatcher::{WebDispatcher, WebDispatcherMode, WebDispatcherSingleThreadedReason};
#[cfg(target_family = "wasm")]
pub use display::WebDisplay;
#[cfg(target_family = "wasm")]
pub use http_client::FetchHttpClient;
#[cfg(target_family = "wasm")]
pub use keyboard::WebKeyboardLayout;
#[cfg(target_family = "wasm")]
pub use logging::init_logging;
#[cfg(target_family = "wasm")]
pub use platform::{WebPlatform, WebPlatformOptions};
#[cfg(target_family = "wasm")]
pub use window::WebWindow;
