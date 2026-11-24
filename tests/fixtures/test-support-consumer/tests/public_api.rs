use open_gpui::{App, AsyncApp, BackgroundExecutor};

#[test]
fn ordinary_identity_apis_are_available_without_test_support() {
    let _: fn(&App) -> bool = App::is_test;
    let _: fn(&AsyncApp) -> bool = AsyncApp::is_test;
    let _: fn(&BackgroundExecutor) -> bool = BackgroundExecutor::is_test;
    let _: fn(&open_gpui_scheduler::LocalExecutor) -> bool =
        open_gpui_scheduler::LocalExecutor::is_test;
    let _: fn(&open_gpui_scheduler::BackgroundExecutor) -> bool =
        open_gpui_scheduler::BackgroundExecutor::is_test;
    let _: fn() -> bool = open_gpui_scheduler::test_scheduler_created;
}

#[test]
fn ordinary_home_directory_consumer_runs_with_the_selected_features() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_open-gpui-test-support-consumer"))
        .output()
        .expect("the downstream home-directory consumer must launch");
    assert!(
        output.status.success(),
        "the downstream application failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "test-support")]
mod deterministic {
    use super::*;
    use open_gpui::{AppContext as _, Empty, LeakDetectorSnapshot, TestAppContext};
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    fn tracking_expected() -> bool {
        match std::env::var("GPUI_TEST_EXPECT_LEAK_DETECTION").as_deref() {
            Ok("on") => true,
            Ok("off") => {
                assert!(!cfg!(feature = "leak-detection"));
                false
            }
            Err(std::env::VarError::NotPresent) => {
                cfg!(feature = "leak-detection")
                    || option_env!("GPUI_LEAK_DETECTION").is_some_and(|value| !value.is_empty())
            }
            _ => panic!("set GPUI_TEST_EXPECT_LEAK_DETECTION to on or off for this build"),
        }
    }

    #[open_gpui::test]
    fn actual_public_test_context_preserves_handles_snapshots_and_window_ownership(
        cx: &mut TestAppContext,
    ) {
        assert!(cx.read(App::is_test));
        assert!(cx.to_async().is_test());
        assert!(cx.executor().is_test());
        assert!(cx.executor().scheduler_executor().is_test());
        assert!(cx.foreground_executor().scheduler_executor().is_test());
        assert!(open_gpui_scheduler::test_scheduler_created());
        let tracking = tracking_expected();
        let empty = LeakDetectorSnapshot::default();
        cx.read(|app| app.assert_no_new_leaks(&empty));
        let releases = Rc::new(Cell::new(0));
        let entity = cx.new({
            let releases = releases.clone();
            move |cx| {
                cx.on_release(move |value, app| {
                    assert!(app.is_test());
                    assert_eq!(*value, 42_usize);
                    releases.set(releases.get() + 1);
                })
                .detach();
                1_usize
            }
        });
        let weak = entity.downgrade();
        let baseline = cx.read(App::leak_detector_snapshot);
        let erased = entity.clone().into_any();
        let erased_weak = erased.downgrade();
        cx.read(|app| app.assert_no_new_leaks(&baseline));
        let task = cx.spawn({
            let entity = entity.clone();
            async move |app| {
                assert!(app.is_test());
                let value = app.background_executor().spawn(async { 42_usize }).await;
                app.update(|app| entity.write(app, value));
            }
        });
        cx.foreground_executor().block_test(task);
        assert_eq!(entity.read_with(cx, |value, _| *value), 42);
        drop(entity);
        assert!(weak.upgrade().is_some());
        assert_eq!(
            catch_unwind(AssertUnwindSafe(|| {
                cx.read(|app| app.assert_no_new_leaks(&empty));
            }))
            .is_err(),
            tracking
        );
        assert!(catch_unwind(AssertUnwindSafe(|| weak.assert_released())).is_err());
        drop(erased);
        assert!(weak.upgrade().is_none());
        assert_eq!(releases.get(), 0);
        assert!(catch_unwind(AssertUnwindSafe(|| erased_weak.assert_released())).is_err());
        cx.update(|_| ());
        weak.assert_released();
        erased_weak.assert_released();
        assert_eq!(releases.get(), 1);
        cx.read(|app| app.assert_no_new_leaks(&empty));

        let window = cx.add_window(|_, _| Empty);
        let root = window
            .root(cx)
            .expect("a real Test window must own its root");
        let root_weak = root.downgrade();
        drop(root);
        cx.update(|_| ());
        assert!(root_weak.upgrade().is_some());
        cx.update_window(window.into(), |_, window, app| window.remove_window(app))
            .expect("the public Test window must close");
        cx.run_until_parked();
        cx.update(|_| ());
        root_weak.assert_released();
        cx.read(|app| app.assert_no_new_leaks(&empty));
    }
}
