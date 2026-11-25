use super::LinuxPlatform;
use open_gpui::Platform as _;

#[test]
fn headless_idle_sleep_prevention_is_unsupported() {
    let platform = LinuxPlatform::new(open_gpui::WindowingModes::HEADLESS);
    let error = smol::block_on(platform.prevent_idle_sleep("headless activity"))
        .err()
        .expect("headless acquisition should be unsupported");

    assert_eq!(
        error.to_string(),
        "Idle sleep prevention for \"headless activity\" requires a Linux windowing backend"
    );
}

#[cfg(any(feature = "wayland", feature = "x11"))]
mod portal {
    use super::super::await_idle_sleep_prevention;
    use anyhow::{Result, anyhow};
    use futures::channel::oneshot;
    use open_gpui::{ActivityGuard, TestAppContext};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering::SeqCst},
        },
        time::Duration,
    };

    #[open_gpui::test]
    async fn guard_passes_through_and_releases_on_drop(cx: &mut TestAppContext) {
        let released = Arc::new(AtomicUsize::new(0));
        let (guard_tx, guard_rx) = oneshot::channel();
        assert!(guard_tx.send(Ok(release_guard(released.clone()))).is_ok());
        let guard = await_idle_sleep_prevention(guard_rx, &cx.background_executor)
            .await
            .expect("acquisition should succeed");

        assert_eq!(released.load(SeqCst), 0);
        drop(guard);
        assert_eq!(released.load(SeqCst), 1);
    }

    #[open_gpui::test]
    async fn acquisition_error_passes_through(cx: &mut TestAppContext) {
        let (guard_tx, guard_rx) = oneshot::channel::<Result<ActivityGuard>>();
        assert!(guard_tx.send(Err(anyhow!("inhibition rejected"))).is_ok());
        assert_eq!(
            await_idle_sleep_prevention(guard_rx, &cx.background_executor)
                .await
                .err()
                .expect("acquisition should fail")
                .to_string(),
            "inhibition rejected"
        );
    }

    #[open_gpui::test]
    async fn abandoned_acquisition_is_an_error(cx: &mut TestAppContext) {
        let (guard_tx, guard_rx) = oneshot::channel::<Result<ActivityGuard>>();
        drop(guard_tx);
        assert_eq!(
            await_idle_sleep_prevention(guard_rx, &cx.background_executor)
                .await
                .err()
                .expect("acquisition should fail")
                .to_string(),
            "Idle sleep prevention request was abandoned"
        );
    }

    #[open_gpui::test]
    async fn late_guard_after_timeout_is_released(cx: &mut TestAppContext) {
        let released = Arc::new(AtomicUsize::new(0));
        let (guard_tx, guard_rx) = oneshot::channel();
        let task = cx.background_executor.spawn({
            let executor = cx.background_executor.clone();
            async move { await_idle_sleep_prevention(guard_rx, &executor).await }
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(9));
        cx.run_until_parked();
        assert!(!guard_tx.is_canceled());
        cx.executor().advance_clock(Duration::from_secs(1));
        assert_eq!(
            task.await
                .err()
                .expect("acquisition should time out")
                .to_string(),
            "Idle sleep prevention acquisition timed out after 10 seconds"
        );

        assert!(guard_tx.is_canceled());
        if let Err(Ok(guard)) = guard_tx.send(Ok(release_guard(released.clone()))) {
            assert_eq!(released.load(SeqCst), 0);
            drop(guard);
        }
        assert_eq!(released.load(SeqCst), 1);
    }

    #[open_gpui::test]
    async fn late_guard_after_receiver_cancellation_is_released(cx: &mut TestAppContext) {
        let released = Arc::new(AtomicUsize::new(0));
        let (guard_tx, guard_rx) = oneshot::channel();
        let task = cx.background_executor.spawn({
            let executor = cx.background_executor.clone();
            async move { await_idle_sleep_prevention(guard_rx, &executor).await }
        });
        cx.run_until_parked();
        assert!(!guard_tx.is_canceled());

        drop(task);
        cx.run_until_parked();
        assert!(guard_tx.is_canceled());
        assert_eq!(released.load(SeqCst), 0);

        guard_tx.send(Ok(release_guard(released.clone()))).ok();
        assert_eq!(released.load(SeqCst), 1);
    }

    fn release_guard(released: Arc<AtomicUsize>) -> ActivityGuard {
        ActivityGuard::new(move || {
            released.fetch_add(1, SeqCst);
        })
    }
}
