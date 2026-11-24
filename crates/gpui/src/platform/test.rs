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

use crate::{
    BackgroundExecutor, ForegroundExecutor, Platform, PlatformHeadlessRenderer, PlatformTextSystem,
    ThreadedDispatcher,
};
use std::{cell::OnceCell, rc::Rc, sync::Arc};

/// Builds a headless platform for testing an ordinary application with real background workers.
///
/// The optional renderer factory supplies a renderer for each opened window.
pub fn headless_test_platform(
    renderer_factory: Option<
        Box<dyn Fn() -> anyhow::Result<Option<Box<dyn PlatformHeadlessRenderer>>>>,
    >,
    text_system: Arc<dyn PlatformTextSystem>,
) -> Rc<dyn Platform> {
    thread_local! {
        static DISPATCHER: OnceCell<Arc<ThreadedDispatcher>> = const { OnceCell::new() };
    }
    let dispatcher = DISPATCHER.with(|cell| {
        cell.get_or_init(|| Arc::new(ThreadedDispatcher::new()))
            .clone()
    });
    TestPlatform::with_platform(
        BackgroundExecutor::new(dispatcher.clone()),
        ForegroundExecutor::new(dispatcher),
        text_system,
        renderer_factory,
    )
}
