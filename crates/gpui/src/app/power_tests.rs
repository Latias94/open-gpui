use crate::{Subscription, TestAppContext};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[crate::test]
fn sleep_and_wake_subscribers_have_independent_lifetimes(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let (sleep_one, sleep_two, wake_one, wake_two) = cx.read(|app| {
        let subscribe_sleep = |name| {
            let observed = observed.clone();
            app.on_system_sleep(move |_| observed.borrow_mut().push(name))
        };
        let subscribe_wake = |name| {
            let observed = observed.clone();
            app.on_system_wake(move |_| observed.borrow_mut().push(name))
        };
        (
            subscribe_sleep("sleep-one"),
            subscribe_sleep("sleep-two"),
            subscribe_wake("wake-one"),
            subscribe_wake("wake-two"),
        )
    });
    cx.simulate_system_sleep();
    cx.simulate_system_sleep();
    cx.simulate_system_wake();
    assert_eq!(
        observed.take(),
        [
            "sleep-one",
            "sleep-two",
            "sleep-one",
            "sleep-two",
            "wake-one",
            "wake-two"
        ]
    );
    drop(sleep_one);
    drop(wake_two);
    cx.simulate_system_sleep();
    cx.simulate_system_wake();
    assert_eq!(observed.take(), ["sleep-two", "wake-one"]);
    sleep_two.detach();
    drop(wake_one);
    cx.simulate_system_sleep();
    cx.simulate_system_wake();
    assert_eq!(observed.take(), ["sleep-two"]);
}

#[crate::test]
fn native_sleep_wake_boundaries_survive_a_busy_app_without_coalescing(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscriptions = cx.read(|app| {
        let sleep = app.on_system_sleep({
            let observed = observed.clone();
            move |app| {
                observed.borrow_mut().push("sleep");
                let observed = observed.clone();
                app.defer(move |_| observed.borrow_mut().push("sleep-effect"));
            }
        });
        let wake = app.on_system_wake({
            let observed = observed.clone();
            move |_| observed.borrow_mut().push("wake")
        });
        Subscription::join(sleep, wake)
    });
    let platform = cx.test_platform();
    cx.update(|_| {
        platform.simulate_system_sleep();
        platform.simulate_system_wake();
        platform.simulate_system_sleep();
        platform.simulate_system_wake();
        assert!(observed.borrow().is_empty());
    });
    cx.run_until_parked();
    assert_eq!(
        observed.take(),
        [
            "sleep",
            "sleep-effect",
            "wake",
            "sleep",
            "sleep-effect",
            "wake"
        ]
    );
}

#[crate::test]
fn lifecycle_subscribers_can_replace_their_own_subscription_and_enqueue_later_events(
    cx: &mut TestAppContext,
) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let current = Rc::new(RefCell::new(None::<Subscription>));
    let ingress = cx.app.native_application_ingress();
    let subscription = cx.read(|app| {
        app.on_system_sleep({
            let current = current.clone();
            let observed = observed.clone();
            let ingress = ingress.clone();
            move |app| {
                observed.borrow_mut().push("first-sleep");
                current.borrow_mut().take();
                let observed = observed.clone();
                *current.borrow_mut() = Some(app.on_system_sleep(move |_| {
                    observed.borrow_mut().push("next-sleep");
                }));
                ingress.system_wake();
            }
        })
    });
    *current.borrow_mut() = Some(subscription);
    let _wake = cx.read(|app| {
        app.on_system_wake({
            let observed = observed.clone();
            move |_| observed.borrow_mut().push("wake")
        })
    });
    cx.simulate_system_sleep();
    assert_eq!(observed.take(), ["first-sleep", "wake"]);
    cx.simulate_system_sleep();
    assert_eq!(observed.take(), ["next-sleep"]);
    current.borrow_mut().take();
}

#[crate::test]
fn cross_thread_lifecycle_events_preserve_their_order_before_a_later_local_callback(
    cx: &mut TestAppContext,
) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscriptions = cx.read(|app| {
        Subscription::join(
            app.on_system_sleep({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push("sleep")
            }),
            app.on_system_wake({
                let observed = observed.clone();
                move |_| observed.borrow_mut().push("wake")
            }),
        )
    });
    super::Application(cx.app.clone()).on_reopen({
        let observed = observed.clone();
        move |_| observed.borrow_mut().push("reopen")
    });
    let ingress = cx.app.native_application_ingress();
    let platform = cx.test_platform();
    cx.update(|_| {
        std::thread::spawn(move || {
            ingress.system_sleep();
            ingress.system_wake();
            ingress.system_sleep();
        })
        .join()
        .expect("the native lifecycle source must complete");
        platform.simulate_reopen();
        assert!(observed.borrow().is_empty());
    });
    cx.run_until_parked();
    assert_eq!(observed.take(), ["sleep", "wake", "sleep", "reopen"]);
}

#[crate::test]
fn panicking_lifecycle_observer_keeps_other_subscriptions_for_the_next_event(
    cx: &mut TestAppContext,
) {
    let delivered = Rc::new(Cell::new(0));
    let failed = cx.read(|app| app.on_system_sleep(|_| panic!("injected sleep observer panic")));
    let _survivor = cx.read(|app| {
        app.on_system_sleep({
            let delivered = delivered.clone();
            move |_| delivered.set(delivered.get() + 1)
        })
    });
    assert!(catch_unwind(AssertUnwindSafe(|| cx.simulate_system_sleep())).is_err());
    assert_eq!(delivered.get(), 0);
    drop(failed);
    cx.simulate_system_sleep();
    assert_eq!(delivered.get(), 1);
}

#[crate::test]
fn dropped_lifecycle_subscriptions_release_captured_owners(cx: &mut TestAppContext) {
    let owner = Rc::new(());
    let weak = Rc::downgrade(&owner);
    let subscription = cx.read(|app| {
        app.on_system_wake(move |_| {
            let _ = Rc::strong_count(&owner);
        })
    });
    assert!(weak.upgrade().is_some());
    drop(subscription);
    assert!(weak.upgrade().is_none());
    cx.simulate_system_wake();
}

#[crate::test]
fn queued_lifecycle_events_are_discarded_after_native_quit(cx: &mut TestAppContext) {
    let delivered = Rc::new(Cell::new(0));
    let _subscriptions = cx.read(|app| {
        Subscription::join(
            app.on_system_sleep({
                let delivered = delivered.clone();
                move |_| delivered.set(delivered.get() + 1)
            }),
            app.on_system_wake({
                let delivered = delivered.clone();
                move |_| delivered.set(delivered.get() + 1)
            }),
        )
    });
    let cell = cx.app.clone();
    let platform = cx.test_platform();
    cx.update(|_| {
        cell.enqueue_quit_for_test();
        platform.simulate_system_sleep();
        platform.simulate_system_wake();
    });
    cx.run_until_parked();
    assert_eq!(delivered.get(), 0);
}
