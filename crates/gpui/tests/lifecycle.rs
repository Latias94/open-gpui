#![cfg(feature = "test-support")]

use open_gpui::{Empty, Subscription, TestAppContext, WindowVisibility};
use std::{cell::RefCell, rc::Rc};

#[open_gpui::test]
fn public_lifecycle_and_window_visibility_use_the_documented_test_context(cx: &mut TestAppContext) {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let handle = cx.add_window(|_, _| Empty);
    let _power = cx.update(|app| {
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
    let window_subscription = handle
        .update(cx, |_, window, _| {
            assert!(window.is_visible());
            let observed = observed.clone();
            window.observe_window_visibility(move |visibility, window, _| {
                assert_eq!(window.visibility(), visibility);
                observed.borrow_mut().push(if visibility.is_visible() {
                    "visible"
                } else {
                    "hidden"
                });
            })
        })
        .expect("the public consumer window must remain live");
    cx.simulate_system_sleep();
    cx.simulate_window_visibility_change(handle.into(), WindowVisibility::Hidden);
    cx.simulate_window_visibility_change(handle.into(), WindowVisibility::Hidden);
    cx.simulate_system_wake();
    cx.simulate_window_visibility_change(handle.into(), WindowVisibility::Visible);
    assert_eq!(observed.take(), ["sleep", "hidden", "wake", "visible"]);
    drop(window_subscription);
    cx.simulate_window_visibility_change(handle.into(), WindowVisibility::Hidden);
    handle
        .update(cx, |_, window, _| assert!(!window.is_visible()))
        .unwrap();
    assert!(observed.borrow().is_empty());
}

#[open_gpui::test]
fn public_visual_context_forwards_visibility_and_preserves_observer_lifetime(
    cx: &mut TestAppContext,
) {
    let visual = cx.add_empty_window();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let subscription = visual.update(|window, _| {
        let observed = observed.clone();
        window.observe_window_visibility(move |visibility, window, _| {
            assert_eq!(window.visibility(), visibility);
            observed.borrow_mut().push(visibility);
        })
    });
    visual.simulate_visibility_change(WindowVisibility::Hidden);
    visual.simulate_visibility_change(WindowVisibility::Visible);
    assert_eq!(
        observed.take(),
        [WindowVisibility::Hidden, WindowVisibility::Visible]
    );
    drop(subscription);
    visual.simulate_visibility_change(WindowVisibility::Hidden);
    assert!(observed.borrow().is_empty());
    visual.update(|window, _| assert_eq!(window.visibility(), WindowVisibility::Hidden));
}
