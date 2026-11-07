use open_gpui::{
    App, AsyncApp, FrameRequestSource, PlatformWindow, PlatformWindowPresentOutcome,
    RequestFrameOptions, Window, WindowInitialPresentationStatus, WindowPresentAttemptFacts,
    WindowPresentationFacts,
};
use std::rc::Rc;

#[test]
fn downstream_platform_scheduling_contract_typechecks_without_constructing_a_backend() {
    let _backend_type_only: fn(&dyn PlatformWindow) -> Option<Rc<dyn Fn()>> = |window| {
        window.schedule_frame();
        let waker: Option<Rc<dyn Fn()>> = window.frame_waker();
        if let Some(waker) = waker.as_ref() {
            let shared: Rc<dyn Fn()> = Rc::clone(waker);
            shared();
            waker();
        }
        let callback: Box<dyn Fn(RequestFrameOptions)> = Box::new(|options| {
            let _: bool = options.force_render;
            let _: bool = options.require_presentation;
            let _: FrameRequestSource = options.signal_source;
            let _captured_time = options.signal_at;
        });
        window.on_request_frame(callback);
        waker
    };
}

#[test]
fn downstream_refresh_and_complete_presentation_facts_typecheck_without_starting_an_app() {
    let _window_type_only: fn(&mut Window, &mut App, &AsyncApp) -> WindowPresentationFacts =
        |window, app, async_app| {
            window.on_next_frame(|window, _| {
                window.on_next_frame(|_, _| {});
            });
            window.refresh();
            app.refresh_windows();
            let _: () = async_app.refresh();

            let facts = window.presentation_facts();
            let WindowPresentationFacts {
                native_created,
                frame_accepted_generation,
                present_submitted_generation,
                non_empty_presented_generation,
                latest_present_attempt,
                initial_presentation,
                native_visible,
            } = facts;
            let _: bool = native_created;
            let _: Option<u64> = frame_accepted_generation;
            let _: Option<u64> = present_submitted_generation;
            let _: Option<u64> = non_empty_presented_generation;
            let _: WindowInitialPresentationStatus = initial_presentation;
            let _: bool = native_visible;
            if let Some(WindowPresentAttemptFacts {
                generation,
                outcome,
                contained_valid_primitives,
            }) = latest_present_attempt
            {
                let _: u64 = generation;
                let _: PlatformWindowPresentOutcome = outcome;
                let _: bool = contained_valid_primitives;
            }
            facts
        };
}
