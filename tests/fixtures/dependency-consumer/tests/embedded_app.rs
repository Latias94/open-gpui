//! Type-checks the public embedded App API without enabling test-support.
//! These consumers do not start an Application, open a Window, or qualify native loop support.

use open_gpui::{App, Application, ApplicationHandle, AsyncApp};

#[test]
fn downstream_consumes_application_and_returns_a_strong_handle() {
    let _: fn(Application, fn(&mut App)) -> ApplicationHandle =
        Application::run_embedded::<fn(&mut App)>;
    let _: fn(&ApplicationHandle) -> AsyncApp = ApplicationHandle::to_async;
    let _consumer = |application: Application| {
        let launch_payload = String::from("owned launch payload");
        let handle: ApplicationHandle = application.run_embedded(move |cx| {
            cx.defer(move |_| drop(launch_payload));
        });
        let weak_context: AsyncApp = handle.to_async();
        (handle, weak_context)
    };
}

#[test]
fn downstream_updates_through_a_shared_handle_with_a_generic_owned_result() {
    let _: fn(&ApplicationHandle, fn(&mut App) -> String) -> String =
        ApplicationHandle::update::<String>;
    let _consumer = |handle: &ApplicationHandle| {
        let payload = String::from("owned update result");
        let returned: (String, usize) = handle.update(move |cx| (payload, cx.windows().len()));
        let weak_context: AsyncApp = handle.to_async();
        (returned, weak_context)
    };
}
