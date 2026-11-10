use super::*;
use crate::{AnyWindowHandle, AppContext, Empty, QuitMode, TestAppContext, px, size};
use std::sync::Once;

thread_local! {
    static TERMINAL_ERRORS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

struct NativeTerminalLogger;

impl log::Log for NativeTerminalLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() == log::Level::Error && metadata.target() == "open_gpui::app::cell"
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            TERMINAL_ERRORS.with(|errors| {
                if let Some(errors) = errors.borrow_mut().as_mut() {
                    errors.push(record.args().to_string());
                }
            });
        }
    }

    fn flush(&self) {}
}

struct TerminalErrorCapture;

impl TerminalErrorCapture {
    fn start() -> Self {
        static LOGGER: NativeTerminalLogger = NativeTerminalLogger;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            log::set_logger(&LOGGER).expect("native terminal tests require their process logger");
            log::set_max_level(log::LevelFilter::Error);
        });
        TERMINAL_ERRORS.with(|errors| {
            assert!(errors.borrow_mut().replace(Vec::new()).is_none());
        });
        Self
    }

    fn take(&self) -> Vec<String> {
        TERMINAL_ERRORS.with(|errors| {
            std::mem::take(
                errors
                    .borrow_mut()
                    .as_mut()
                    .expect("capture must be active"),
            )
        })
    }
}

impl Drop for TerminalErrorCapture {
    fn drop(&mut self) {
        TERMINAL_ERRORS.with(|errors| {
            errors.borrow_mut().take();
        });
    }
}

#[crate::test]
fn settled_native_terminal_accepts_queued_closed_without_error(cx: &mut TestAppContext) {
    let errors = TerminalErrorCapture::start();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let anchor: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let dependent: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    let anchor_platform = cx.test_window(anchor);
    let dependent_platform = cx.test_window(dependent);
    let anchor_terminal = cx.hold_window_native_terminal(anchor);
    let dependent_terminal = cx.hold_window_native_terminal(dependent);
    let generation = cx.update(|app| {
        app.reserve_native_captured_drag_start()
            .token()
            .generation()
    });
    let completions = Rc::new(RefCell::new(Vec::new()));
    let (_, release_barrier) = cx.app.reserve_native_captured_drag_release(
        dependent.window_id(),
        generation,
        Box::new({
            let completions = completions.clone();
            move |barrier, terminal, _| {
                completions.borrow_mut().push((barrier, terminal));
            }
        }),
    );
    cx.update(|app| {
        app.register_native_window_retirement_dependencies(
            anchor.window_id(),
            [dependent.window_id()],
        )
        .expect("the anchor must wait for the dependent native terminal");
        anchor
            .update(app, |_, window, app| window.remove_window(app))
            .expect("the anchor must close logically");
        dependent
            .update(app, |_, window, app| window.remove_window(app))
            .expect("the dependent must close logically");
    });
    cx.run_until_parked();
    assert_eq!(anchor_platform.presentation_shutdown_counts().2, 0);
    assert_eq!(dependent_platform.presentation_shutdown_counts(), (1, 1, 1));
    assert!(completions.borrow().is_empty());
    let shutdown = cx
        .app
        .native_window_retirements
        .borrow()
        .get(&dependent.window_id())
        .expect("the held callback must retain the exact retirement barrier")
        .presentation_shutdown
        .clone()
        .expect("the dispatched retirement must retain its prepared presentation authority");
    let ticket = shutdown.ticket().clone();

    cx.app.settle_native_window_terminal(dependent.window_id());
    assert!(
        !cx.app
            .native_window_retirements
            .borrow()
            .contains_key(&dependent.window_id())
    );
    assert!(ticket.same_authority(shutdown.ticket()));
    let snapshot = ticket.snapshot();
    assert_eq!(snapshot.window_id(), dependent.window_id());
    assert!(snapshot.quiesced());
    assert!(snapshot.native_terminal());
    assert!(!snapshot.protocol_violation());
    cx.run_until_parked();
    assert_eq!(
        completions.borrow().as_slice(),
        &[(
            release_barrier,
            NativeCapturedDragReleaseTerminal::NativeWindowTerminal
        )]
    );
    assert!(cx.app.pointer_capture_release_barriers_are_clear());
    assert!(
        cx.app
            .native_window_retirement_dependencies
            .borrow()
            .is_empty()
    );
    assert_eq!(anchor_platform.presentation_shutdown_counts().2, 1);

    cx.update(|app| app.shutdown());
    cx.run_until_parked();
    assert!(cx.app.shutdown_fence.borrow().is_some());
    assert!(dependent_terminal.release());
    cx.run_until_parked();
    assert_eq!(completions.borrow().len(), 1);
    assert!(cx.app.shutdown_fence.borrow().is_some());
    assert!(anchor_terminal.release());
    cx.run_until_parked();
    assert!(cx.app.native_window_retirement_barriers_are_clear());
    assert!(cx.app.pointer_capture_release_barriers_are_clear());
    assert!(cx.app.shutdown_fence.borrow().is_none());
    cx.update(|app| assert_eq!(app.window_open_barrier_depth, 0));
    assert_eq!(
        errors.take(),
        Vec::<String>::new(),
        "a queued Closed must accept an already settled terminal"
    );
}

#[crate::test]
fn native_terminal_without_authority_still_reports_an_error(cx: &mut TestAppContext) {
    let errors = TerminalErrorCapture::start();
    let window_id = WindowId::from(7);
    let expected = format!(
        "native window terminal was observed without a registered presentation-shutdown authority for window {:?}",
        window_id,
    );

    cx.app.settle_native_window_terminal(window_id);
    assert_eq!(errors.take(), vec![expected.clone()]);
    cx.app.settle_native_window_terminal(window_id);
    assert_eq!(errors.take(), vec![expected]);
}

#[crate::test]
fn native_terminal_before_quiescence_is_not_a_successful_settlement(cx: &mut TestAppContext) {
    let errors = TerminalErrorCapture::start();
    cx.update(|app| app.set_quit_mode(QuitMode::Explicit));
    let window: AnyWindowHandle = cx
        .open_window(size(px(320.0), px(200.0)), |_, _| Empty)
        .into();
    cx.test_window(window).block_presentation_shutdown(true);
    cx.update_window(window, |_, window, app| window.remove_window(app))
        .expect("the window must close logically before presentation quiesces");
    cx.run_until_parked();
    let shutdown = cx
        .app
        .native_window_retirements
        .borrow()
        .get(&window.window_id())
        .expect("rejected quiescence must retain its retirement authority")
        .presentation_shutdown
        .clone()
        .expect("rejected quiescence must retain its prepared presentation authority");
    assert!(!shutdown.snapshot().quiesced());

    cx.app.settle_native_window_terminal(window.window_id());
    let snapshot = shutdown.snapshot();
    assert!(snapshot.native_terminal());
    assert!(!snapshot.quiesced());
    assert!(snapshot.protocol_violation());
    assert_eq!(
        errors.take(),
        vec![format!(
            "native window terminal preceded presentation quiescence for window {:?}, generation {}",
            window.window_id(),
            snapshot.generation(),
        )]
    );
    cx.app.settle_native_window_terminal(window.window_id());
    assert_eq!(
        errors.take(),
        vec![format!(
            "native window terminal was observed without a registered presentation-shutdown authority for window {:?}",
            window.window_id(),
        )]
    );
    assert!(!shutdown.ticket().acknowledge_quiesced());
}
