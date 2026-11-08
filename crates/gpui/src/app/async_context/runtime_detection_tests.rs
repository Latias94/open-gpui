use crate::{
    App, BackgroundExecutor, ForegroundExecutor, PlatformDispatcher, TestDispatcher, TestPlatform,
    ThreadedDispatcher,
};
use std::{rc::Rc, sync::Arc};

#[test]
fn async_app_detection_does_not_borrow_or_retain_the_app() {
    let threaded_dispatcher = Arc::new(ThreadedDispatcher::new());
    let test_dispatcher = Arc::new(TestDispatcher::new(17));
    assert!(open_gpui_scheduler::test_scheduler_created());

    let dispatchers: [(Arc<dyn PlatformDispatcher>, bool); 2] = [
        (threaded_dispatcher.clone(), false),
        (test_dispatcher.clone(), true),
    ];
    for (dispatcher, expected) in dispatchers {
        let background = BackgroundExecutor::new(dispatcher.clone());
        let foreground = ForegroundExecutor::new(dispatcher);
        let platform = TestPlatform::new(background, foreground);
        let app = App::new_app(
            platform,
            Arc::new(()),
            open_gpui_http_client::FakeHttpClient::with_404_response(),
        );
        let weak_app = Rc::downgrade(&app);
        let owner_count = Rc::strong_count(&app);
        let async_app = app.borrow().to_async();
        let cloned_async_app = async_app.clone();

        let borrowed_app = app.borrow_mut();
        assert_eq!(async_app.is_test(), expected);
        assert_eq!(cloned_async_app.is_test(), expected);
        assert_eq!(async_app.background_executor().is_test(), expected);
        assert_eq!(Rc::strong_count(&app), owner_count);
        drop(borrowed_app);

        drop(app);
        assert!(weak_app.upgrade().is_none());
        assert_eq!(async_app.is_test(), expected);
        assert_eq!(cloned_async_app.is_test(), expected);
    }

    test_dispatcher.run_until_parked();
    threaded_dispatcher.run_until_idle();
}
