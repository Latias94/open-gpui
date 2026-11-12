use crate::{AppLifecyclePhase, Subscription, TestAppContext};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, PartialEq)]
enum Observed {
    Lifecycle(AppLifecyclePhase),
    MemoryWarning,
}

#[crate::test]
fn lifecycle_and_memory_notifications_wait_for_the_app_and_keep_fifo(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscriptions = cx.read(|app| {
        Subscription::join(
            app.on_app_lifecycle({
                let observed = observed.clone();
                move |phase, _| observed.borrow_mut().push(Observed::Lifecycle(phase))
            }),
            app.on_memory_warning({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push(Observed::MemoryWarning)
            }),
        )
    });
    let platform = cx.test_platform();
    cx.update(|_| {
        platform.simulate_app_lifecycle(AppLifecyclePhase::Inactive);
        platform.simulate_memory_warning();
        platform.simulate_app_lifecycle(AppLifecyclePhase::Background);
        platform.simulate_app_lifecycle(AppLifecyclePhase::Background);
        platform.simulate_app_lifecycle(AppLifecyclePhase::Foreground);
        platform.simulate_memory_warning();
        platform.simulate_app_lifecycle(AppLifecyclePhase::Active);
        platform.simulate_app_lifecycle(AppLifecyclePhase::Disconnected);
        assert!(observed.borrow().is_empty());
    });
    cx.run_until_parked();
    assert_eq!(
        observed.take(),
        [
            Observed::Lifecycle(AppLifecyclePhase::Inactive),
            Observed::MemoryWarning,
            Observed::Lifecycle(AppLifecyclePhase::Background),
            Observed::Lifecycle(AppLifecyclePhase::Background),
            Observed::Lifecycle(AppLifecyclePhase::Foreground),
            Observed::MemoryWarning,
            Observed::Lifecycle(AppLifecyclePhase::Active),
            Observed::Lifecycle(AppLifecyclePhase::Disconnected),
        ]
    );
}

#[crate::test]
fn lifecycle_and_memory_subscriptions_release_independently(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let (lifecycle, memory) = cx.read(|app| {
        (
            app.on_app_lifecycle({
                let observed = observed.clone();
                move |_, _| observed.borrow_mut().push("lifecycle")
            }),
            app.on_memory_warning({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push("memory")
            }),
        )
    });
    cx.test_platform()
        .simulate_app_lifecycle(AppLifecyclePhase::Active);
    cx.test_platform().simulate_memory_warning();
    cx.run_until_parked();
    assert_eq!(observed.take(), ["lifecycle", "memory"]);
    drop(lifecycle);
    cx.test_platform()
        .simulate_app_lifecycle(AppLifecyclePhase::Inactive);
    cx.test_platform().simulate_memory_warning();
    cx.run_until_parked();
    assert_eq!(observed.take(), ["memory"]);
    drop(memory);
    cx.test_platform().simulate_memory_warning();
    cx.run_until_parked();
    assert!(observed.borrow().is_empty());
}

#[crate::test]
fn lifecycle_callbacks_can_enqueue_another_native_notification(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let simulator = cx.clone();
    let _subscriptions = cx.read(|app| {
        Subscription::join(
            app.on_app_lifecycle({
                let observed = observed.clone();
                move |phase, _| {
                    observed.borrow_mut().push(Observed::Lifecycle(phase));
                    if phase == AppLifecyclePhase::Background {
                        simulator.test_platform().simulate_memory_warning();
                    }
                }
            }),
            app.on_memory_warning({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push(Observed::MemoryWarning)
            }),
        )
    });
    cx.test_platform()
        .simulate_app_lifecycle(AppLifecyclePhase::Background);
    cx.run_until_parked();
    assert_eq!(
        observed.take(),
        [
            Observed::Lifecycle(AppLifecyclePhase::Background),
            Observed::MemoryWarning
        ]
    );
}
