use open_gpui::{
    App, Context, NativeAppNotificationHandler, Platform, PlatformWindow, Subscription, Window,
    WindowVisibility,
};
use std::{cell::Cell, rc::Rc, sync::Arc};

#[test]
fn downstream_app_lifecycle_subscriptions_typecheck_without_starting_app() {
    let _: fn(&App, fn(&mut App)) -> Subscription = App::on_system_sleep::<fn(&mut App)>;
    let _: fn(&App, fn(&mut App)) -> Subscription = App::on_system_wake::<fn(&mut App)>;
    let _api_only = |app: &App| {
        let mut sleep_windows = Vec::new();
        let sleep: Subscription = app.on_system_sleep(move |app: &mut App| {
            sleep_windows.push(app.windows().len());
        });
        let mut wake_windows = Vec::new();
        let wake: Subscription = app.on_system_wake(move |app: &mut App| {
            wake_windows.push(app.windows().len());
        });
        let another_wake: Subscription = app.on_system_wake(|_: &mut App| {});
        (sleep, wake, another_wake)
    };
}

#[test]
fn downstream_window_and_entity_visibility_typecheck_without_opening_window() {
    let _: fn(&Window) -> WindowVisibility = Window::visibility;
    let _: fn(&Window) -> bool = Window::is_visible;
    let _api_only = |window: &mut Window, cx: &Context<Vec<WindowVisibility>>| {
        let shared_window: &Window = window;
        let mut observed = Vec::new();
        let window_subscription: Subscription = shared_window.observe_window_visibility(
            move |visibility: WindowVisibility, window: &mut Window, app: &mut App| {
                observed.push(visibility);
                let _: WindowVisibility = window.visibility();
                let _: bool = window.is_visible();
                let _: &mut App = app;
            },
        );
        let entity_subscription: Subscription = cx.observe_window_visibility(
            window,
            |entity: &mut Vec<WindowVisibility>,
             visibility: WindowVisibility,
             window: &mut Window,
             cx: &mut Context<Vec<WindowVisibility>>| {
                entity.push(visibility);
                let _: WindowVisibility = window.visibility();
                let _: bool = window.is_visible();
                cx.notify();
            },
        );
        (window_subscription, entity_subscription)
    };
}

#[test]
fn downstream_backend_lifecycle_contract_typechecks_without_constructing_platform() {
    let _api_only = |platform: &dyn Platform, window: &dyn PlatformWindow| {
        let notification: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let notification: NativeAppNotificationHandler = notification;
        platform.on_system_sleep(Arc::clone(&notification));
        platform.on_system_wake(notification);

        let initial: WindowVisibility = window.visibility();
        let mut observed = Vec::new();
        let callback: Box<dyn FnMut(WindowVisibility)> = Box::new(move |visibility| {
            observed.push(visibility);
        });
        window.on_visibility_change(callback);
        initial
    };
}

#[test]
fn downstream_window_visibility_maps_visible_and_hidden() {
    assert!(WindowVisibility::Visible.is_visible());
    assert!(!WindowVisibility::Hidden.is_visible());
}

#[test]
fn downstream_subscription_cancels_once_on_drop() {
    let cancellations = Rc::new(Cell::new(0));
    let subscription = Subscription::new({
        let cancellations = Rc::clone(&cancellations);
        move || cancellations.set(cancellations.get() + 1)
    });
    assert_eq!(cancellations.get(), 0);
    assert_eq!(Rc::strong_count(&cancellations), 2);

    drop(subscription);

    assert_eq!(cancellations.get(), 1);
    assert_eq!(Rc::strong_count(&cancellations), 1);
}

#[test]
fn downstream_subscription_detach_skips_cancellation_and_releases_capture() {
    let cancellations = Rc::new(Cell::new(0));
    let subscription = Subscription::new({
        let cancellations = Rc::clone(&cancellations);
        move || cancellations.set(cancellations.get() + 1)
    });
    assert_eq!(cancellations.get(), 0);
    assert_eq!(Rc::strong_count(&cancellations), 2);

    subscription.detach();

    assert_eq!(cancellations.get(), 0);
    assert_eq!(Rc::strong_count(&cancellations), 1);
}

#[test]
fn downstream_joined_subscriptions_cancel_both_on_drop() {
    let cancellations = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let [first, second] = cancellations.each_ref().map(|cancellations| {
        let cancellations = Rc::clone(cancellations);
        Subscription::new(move || cancellations.set(cancellations.get() + 1))
    });
    let joined = Subscription::join(first, second);
    for cancellations in &cancellations {
        assert_eq!(cancellations.get(), 0);
    }

    drop(joined);

    for cancellations in cancellations {
        assert_eq!(cancellations.get(), 1);
        assert_eq!(Rc::strong_count(&cancellations), 1);
    }
}

#[test]
fn downstream_joined_subscriptions_detach_both() {
    let cancellations = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    let [first, second] = cancellations.each_ref().map(|cancellations| {
        let cancellations = Rc::clone(cancellations);
        Subscription::new(move || cancellations.set(cancellations.get() + 1))
    });
    let joined = Subscription::join(first, second);
    for cancellations in &cancellations {
        assert_eq!(cancellations.get(), 0);
    }

    joined.detach();

    for cancellations in cancellations {
        assert_eq!(cancellations.get(), 0);
        assert_eq!(Rc::strong_count(&cancellations), 1);
    }
}
