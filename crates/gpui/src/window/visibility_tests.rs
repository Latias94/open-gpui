use crate::{
    AppContext as _, Empty, PlatformWindow as _, Subscription, TestAppContext, WindowVisibility,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[crate::test]
fn visibility_initial_state_and_distinct_transitions_do_not_wake_frames(cx: &mut TestAppContext) {
    let handle = cx.add_window(|window, _| {
        assert_eq!(window.visibility(), WindowVisibility::Visible);
        assert_eq!(window.visibility(), window.platform_window.visibility());
        assert!(window.is_visible());
        Empty
    });
    cx.run_until_parked();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = handle
        .update(cx, {
            let observed = observed.clone();
            move |_, window, _| {
                window.observe_window_visibility(move |visibility, window, _| {
                    assert_eq!(window.visibility(), visibility);
                    assert_eq!(window.is_visible(), visibility.is_visible());
                    observed.borrow_mut().push(visibility);
                })
            }
        })
        .expect("the visibility target should remain live");
    assert!(observed.borrow().is_empty());
    let platform_window = cx.test_window(handle.into());
    let frame_wake_count = platform_window.frame_wake_count();
    let draw_count = platform_window.draw_count();
    let mapped = platform_window.is_visible();

    for visibility in [
        WindowVisibility::Visible,
        WindowVisibility::Hidden,
        WindowVisibility::Hidden,
        WindowVisibility::Visible,
        WindowVisibility::Visible,
    ] {
        platform_window.simulate_visibility_change(visibility);
        cx.run_until_parked();
        handle
            .update(cx, |_, window, _| {
                assert_eq!(window.visibility(), visibility);
                assert_eq!(window.is_visible(), visibility.is_visible());
            })
            .expect("the visibility target should remain live");
        assert_eq!(platform_window.visibility(), visibility);
        assert_eq!(platform_window.is_visible(), mapped);
    }

    assert_eq!(
        *observed.borrow(),
        [WindowVisibility::Hidden, WindowVisibility::Visible]
    );
    assert_eq!(platform_window.frame_wake_count(), frame_wake_count);
    assert_eq!(platform_window.draw_count(), draw_count);
}

#[crate::test]
fn visibility_distinct_transitions_remain_ordered_while_app_is_borrowed(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    cx.run_until_parked();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = handle
        .update(cx, {
            let observed = observed.clone();
            move |_, window, _| {
                window.observe_window_visibility(move |visibility, window, _| {
                    assert_eq!(window.visibility(), visibility);
                    observed.borrow_mut().push(visibility);
                })
            }
        })
        .expect("the visibility target should remain live");
    let platform_window = cx.test_window(handle.into());
    let frame_wake_count = platform_window.frame_wake_count();

    handle
        .update(cx, |_, window, _| {
            for visibility in [
                WindowVisibility::Hidden,
                WindowVisibility::Hidden,
                WindowVisibility::Visible,
                WindowVisibility::Hidden,
                WindowVisibility::Visible,
            ] {
                platform_window.simulate_visibility_change(visibility);
            }
            assert!(observed.borrow().is_empty());
            assert_eq!(window.visibility(), WindowVisibility::Visible);
        })
        .expect("the busy visibility target should remain live");
    cx.run_until_parked();

    assert_eq!(
        *observed.borrow(),
        [
            WindowVisibility::Hidden,
            WindowVisibility::Visible,
            WindowVisibility::Hidden,
            WindowVisibility::Visible,
        ]
    );
    assert_eq!(platform_window.frame_wake_count(), frame_wake_count);
}

#[crate::test]
fn visibility_observer_can_unsubscribe_during_delivery(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let subscription = Rc::new(RefCell::new(None::<Subscription>));
    let first_count = Rc::new(Cell::new(0));
    let surviving_count = Rc::new(Cell::new(0));
    let _surviving_subscription = handle
        .update(cx, |_, window, _| {
            let cancel = subscription.clone();
            let first_count = first_count.clone();
            subscription.replace(Some(window.observe_window_visibility(move |_, _, _| {
                first_count.set(first_count.get() + 1);
                cancel.borrow_mut().take();
            })));
            let surviving_count = surviving_count.clone();
            window.observe_window_visibility(move |_, _, _| {
                surviving_count.set(surviving_count.get() + 1);
            })
        })
        .expect("the visibility target should remain live");
    let platform_window = cx.test_window(handle.into());

    platform_window.simulate_visibility_change(WindowVisibility::Hidden);
    cx.run_until_parked();
    platform_window.simulate_visibility_change(WindowVisibility::Visible);
    cx.run_until_parked();

    assert!(subscription.borrow().is_none());
    assert_eq!(first_count.get(), 1);
    assert_eq!(surviving_count.get(), 2);
}

#[crate::test]
fn visibility_context_observer_releases_unsubscribed_and_dropped_entities(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let entity = cx.new(|_| Vec::<WindowVisibility>::new());
    let retained = Rc::new(());
    let weak_retained = Rc::downgrade(&retained);
    let subscription = handle
        .update(cx, |_, window, app| {
            entity.update(app, |_, cx| {
                cx.observe_window_visibility(window, move |observed, visibility, window, _| {
                    let _ = &retained;
                    assert_eq!(window.visibility(), visibility);
                    observed.push(visibility);
                })
            })
        })
        .expect("the entity's visibility target should remain live");
    let platform_window = cx.test_window(handle.into());
    platform_window.simulate_visibility_change(WindowVisibility::Hidden);
    cx.run_until_parked();
    entity.read_with(cx, |observed, _| {
        assert_eq!(observed, &[WindowVisibility::Hidden]);
    });

    drop(subscription);
    assert!(weak_retained.upgrade().is_none());
    platform_window.simulate_visibility_change(WindowVisibility::Visible);
    cx.run_until_parked();
    entity.read_with(cx, |observed, _| {
        assert_eq!(observed, &[WindowVisibility::Hidden]);
    });

    let retained = Rc::new(());
    let weak_retained = Rc::downgrade(&retained);
    let _subscription = handle
        .update(cx, |_, window, app| {
            entity.update(app, |_, cx| {
                cx.observe_window_visibility(window, move |_, _, _, _| {
                    let _ = &retained;
                    panic!("a released entity must not receive visibility notifications");
                })
            })
        })
        .expect("the entity's visibility target should remain live");
    let weak_entity = entity.downgrade();
    drop(entity);
    cx.update(|_| {});
    assert!(weak_entity.upgrade().is_none());
    assert!(weak_retained.upgrade().is_some());

    platform_window.simulate_visibility_change(WindowVisibility::Hidden);
    cx.run_until_parked();
    assert!(weak_retained.upgrade().is_none());
    platform_window.simulate_visibility_change(WindowVisibility::Visible);
    cx.run_until_parked();
}

#[crate::test]
fn visibility_observer_can_reenter_native_visibility_delivery(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let platform_window = cx.test_window(handle.into());
    let observed = Rc::new(RefCell::new(Vec::new()));
    let reentered = Rc::new(Cell::new(false));
    let _subscription = handle
        .update(cx, {
            let platform_window = platform_window.clone();
            let observed = observed.clone();
            move |_, window, _| {
                window.observe_window_visibility(move |visibility, window, _| {
                    assert_eq!(window.visibility(), visibility);
                    observed.borrow_mut().push(visibility);
                    if !reentered.replace(true) {
                        platform_window.simulate_visibility_change(WindowVisibility::Visible);
                        platform_window.simulate_visibility_change(WindowVisibility::Hidden);
                    }
                })
            }
        })
        .expect("the reentrant visibility target should remain live");

    platform_window.simulate_visibility_change(WindowVisibility::Hidden);
    cx.run_until_parked();
    assert_eq!(
        *observed.borrow(),
        [
            WindowVisibility::Hidden,
            WindowVisibility::Visible,
            WindowVisibility::Hidden,
        ]
    );
}

#[crate::test]
fn visibility_native_callback_replacement_preserves_reentrant_notifications(
    cx: &mut TestAppContext,
) {
    let handle = cx.add_window(|_, _| Empty);
    let platform_window = cx.test_window(handle.into());
    let observed = Rc::new(RefCell::new(Vec::new()));
    platform_window.on_visibility_change(Box::new({
        let platform_window = platform_window.clone();
        let observed = observed.clone();
        move |visibility| {
            assert_eq!(platform_window.visibility(), visibility);
            observed.borrow_mut().push(("original", visibility));
            platform_window.simulate_visibility_change(WindowVisibility::Visible);
            platform_window.simulate_visibility_change(WindowVisibility::Hidden);
            platform_window.on_visibility_change(Box::new({
                let observed = observed.clone();
                move |visibility| observed.borrow_mut().push(("replacement", visibility))
            }));
        }
    }));

    platform_window.simulate_visibility_change(WindowVisibility::Hidden);
    assert_eq!(
        *observed.borrow(),
        [
            ("original", WindowVisibility::Hidden),
            ("replacement", WindowVisibility::Visible),
            ("replacement", WindowVisibility::Hidden),
        ]
    );
}

#[crate::test]
fn visibility_native_callback_survives_unwind(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let platform_window = cx.test_window(handle.into());
    let observed = Rc::new(RefCell::new(Vec::new()));
    platform_window.on_visibility_change(Box::new({
        let observed = observed.clone();
        let panic_once = Cell::new(true);
        move |visibility| {
            observed.borrow_mut().push(visibility);
            assert!(
                !panic_once.replace(false),
                "injected visibility callback panic"
            );
        }
    }));

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            platform_window.simulate_visibility_change(WindowVisibility::Hidden);
        }))
        .is_err()
    );
    platform_window.simulate_visibility_change(WindowVisibility::Visible);
    assert_eq!(
        *observed.borrow(),
        [WindowVisibility::Hidden, WindowVisibility::Visible]
    );
}

struct CallbackDrop(Option<Box<dyn FnOnce()>>);

impl Drop for CallbackDrop {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback();
        }
    }
}

#[crate::test]
fn visibility_native_callback_drop_can_reenter_registration(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let platform_window = cx.test_window(handle.into());
    let observed = Rc::new(RefCell::new(Vec::new()));
    let retired = CallbackDrop(Some(Box::new({
        let platform_window = platform_window.clone();
        let observed = observed.clone();
        move || {
            platform_window.on_visibility_change(Box::new(move |visibility| {
                observed.borrow_mut().push(visibility);
            }));
            platform_window.simulate_visibility_change(WindowVisibility::Hidden);
        }
    })));
    platform_window.on_visibility_change(Box::new(move |_| {
        let _ = &retired;
    }));

    platform_window.on_visibility_change(Box::new(|_| {}));
    assert_eq!(*observed.borrow(), [WindowVisibility::Hidden]);
    platform_window.simulate_visibility_change(WindowVisibility::Visible);
    assert_eq!(
        *observed.borrow(),
        [WindowVisibility::Hidden, WindowVisibility::Visible]
    );
}

#[crate::test]
fn visibility_native_close_retires_an_in_flight_callback(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| Empty);
    let platform_window = cx.test_window(handle.into());
    let dropped = Rc::new(Cell::new(false));
    let deliveries = Rc::new(Cell::new(0));
    let retired = CallbackDrop(Some(Box::new({
        let platform_window = platform_window.clone();
        let dropped = dropped.clone();
        move || {
            assert_eq!(platform_window.visibility(), WindowVisibility::Hidden);
            platform_window.on_visibility_change(Box::new(|_| {
                panic!("a terminal window must not retain a replacement callback");
            }));
            dropped.set(true);
        }
    })));
    platform_window.on_visibility_change(Box::new({
        let platform_window = platform_window.clone();
        let deliveries = deliveries.clone();
        move |_| {
            let _ = &retired;
            deliveries.set(deliveries.get() + 1);
            platform_window.simulate_visibility_change(WindowVisibility::Visible);
            assert!(platform_window.simulate_close());
        }
    }));

    platform_window.simulate_visibility_change(WindowVisibility::Hidden);
    cx.run_until_parked();
    platform_window.simulate_visibility_change(WindowVisibility::Visible);
    assert!(dropped.get());
    assert_eq!(deliveries.get(), 1);
    assert_eq!(platform_window.visibility(), WindowVisibility::Hidden);
}
