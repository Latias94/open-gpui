#![cfg(all(not(target_family = "wasm"), feature = "bench-support"))]

use open_gpui::{Application, NoopTextSystem, QuitMode, bench_platform};
use open_gpui_tokio::Tokio;
use std::sync::Arc;

#[test]
fn standalone_consumer_uses_the_owned_tokio_runtime_through_public_contexts() {
    let app = Application::with_platform(bench_platform(None, Arc::new(NoopTextSystem)))
        .with_quit_mode(QuitMode::Explicit)
        .run_embedded(open_gpui_tokio::init);
    let task = app.update(|cx| Tokio::spawn(cx, async { 42usize }));
    assert_eq!(smol::block_on(task).unwrap(), 42);
}
