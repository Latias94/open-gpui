use crate::{TestAppContext, WindowAppearance};
use std::{cell::RefCell, rc::Rc};

#[crate::test]
fn appearance_requests_forward_exactly_without_synthesizing_platform_state(
    cx: &mut TestAppContext,
) {
    let requests = [
        Some(WindowAppearance::Dark),
        Some(WindowAppearance::VibrantLight),
        Some(WindowAppearance::Light),
        Some(WindowAppearance::VibrantDark),
        Some(WindowAppearance::VibrantDark),
        None,
        None,
    ];
    cx.read(|app| {
        for appearance in requests {
            app.set_window_appearance(appearance);
            assert_eq!(app.window_appearance(), WindowAppearance::Light);
        }
    });

    assert_eq!(
        cx.test_platform()
            .window_appearance_requests
            .borrow()
            .as_slice(),
        requests,
    );
}

#[crate::test]
fn appearance_requests_reach_only_the_owning_application(cx: &mut TestAppContext) {
    let other = cx.new_app();
    cx.read(|app| app.set_window_appearance(Some(WindowAppearance::Dark)));
    assert!(
        other
            .test_platform()
            .window_appearance_requests
            .borrow()
            .is_empty()
    );

    other.read(|app| {
        app.set_window_appearance(Some(WindowAppearance::VibrantLight));
        app.set_window_appearance(None);
    });

    assert_eq!(
        cx.test_platform()
            .window_appearance_requests
            .borrow()
            .as_slice(),
        [Some(WindowAppearance::Dark)],
    );
    assert_eq!(
        other
            .test_platform()
            .window_appearance_requests
            .borrow()
            .as_slice(),
        [Some(WindowAppearance::VibrantLight), None],
    );
}

#[crate::test]
fn busy_app_appearance_notifications_use_existing_ingress_and_allow_observer_requests(
    cx: &mut TestAppContext,
) {
    let window = cx.add_window(|_, _| crate::Empty);
    cx.run_until_parked();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let _subscription = window
        .update(cx, |_, window, _| {
            window.observe_window_appearance({
                let observed = observed.clone();
                move |window, app| {
                    observed.borrow_mut().push(window.appearance());
                    app.set_window_appearance(None);
                }
            })
        })
        .unwrap();
    let asynchronous = cx.read(|app| app.to_async());

    cx.update(|_| {
        asynchronous.enqueue_window_appearance_changed(window.window_id());
        asynchronous.enqueue_window_appearance_changed(window.window_id());
        assert!(observed.borrow().is_empty());
    });
    cx.run_until_parked();

    assert_eq!(observed.borrow().as_slice(), [WindowAppearance::Light]);
    assert_eq!(
        cx.test_platform()
            .window_appearance_requests
            .borrow()
            .as_slice(),
        [None],
    );
}
