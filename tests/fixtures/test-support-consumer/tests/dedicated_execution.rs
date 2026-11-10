#[cfg(feature = "test-support")]
mod deterministic {
    use open_gpui::{DedicatedExecutor, TestAppContext};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Release(Arc<AtomicUsize>);

    impl Drop for Release {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[open_gpui::test]
    async fn dedicated_session_runs_and_cancels_through_public_test_scheduling(
        cx: &mut TestAppContext,
    ) {
        let dedicated = DedicatedExecutor::new(&cx.executor().scheduler_executor());
        let first = dedicated.spawn(async { 20 });
        let second = dedicated.spawn(async { 22 });
        assert_eq!(first.await + second.await, 42);

        let released = Arc::new(AtomicUsize::new(0));
        let guard = Release(released.clone());
        let task = dedicated.spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        dedicated.spawn(async {}).await;
        assert_eq!(released.load(Ordering::SeqCst), 0);
        drop(task);
        dedicated.spawn(async {}).await;
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }
}
