#[cfg(any(test, feature = "test-support"))]
mod dispatcher;
mod display;
mod platform;
mod window;

#[cfg(any(test, feature = "test-support"))]
pub use dispatcher::*;
pub(crate) use display::*;
pub(crate) use platform::*;
pub(crate) use window::*;

#[cfg(any(test, feature = "test-support"))]
pub use platform::{TestScreenCaptureSource, TestScreenCaptureStream};
