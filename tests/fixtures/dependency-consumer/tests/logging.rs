#![cfg(all(feature = "logging", not(target_family = "wasm")))]

#[cfg(feature = "logging-tracy")]
#[global_allocator]
static HOST_ALLOCATOR: std::alloc::System = std::alloc::System;

#[cfg(not(feature = "logging-tracy"))]
#[test]
fn disabled_tracing_does_not_evaluate_span_fields() {
    open_gpui_log::try_init(Some("info".into())).expect("the application owns its logger");
    let evaluated = std::cell::Cell::new(false);
    let span = open_gpui_tracing::info_span!(
        "disabled",
        evaluated = {
            evaluated.set(true);
            true
        }
    );
    span.enter();
    assert!(!evaluated.get());
}

#[cfg(feature = "logging-tracy")]
#[test]
fn host_subscriber_receives_real_list_and_application_spans() {
    use open_gpui::{ListAlignment, ListState, px};
    use std::sync::{Arc, Mutex};
    use tracing::{Id, Subscriber, span::Attributes};
    use tracing_subscriber::{Layer, layer::SubscriberExt as _};

    type SpanNames = Arc<Mutex<Vec<(&'static str, &'static str)>>>;
    struct Names(SpanNames);
    impl<S: Subscriber> Layer<S> for Names {
        fn on_new_span(
            &self,
            attrs: &Attributes<'_>,
            _: &Id,
            _: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let metadata = attrs.metadata();
            self.0
                .lock()
                .unwrap()
                .push((metadata.target(), metadata.name()));
        }
    }

    assert!(!tracy_client::Client::is_running());
    open_gpui_log::try_init(Some("info".into())).expect("the application owns its logger");
    let spans = SpanNames::default();
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(Names(spans.clone())),
    )
    .expect("the application owns its global subscriber");
    let span = open_gpui_tracing::info_span!("edit-list");
    let _entered = span.enter();
    let list = ListState::new(4, ListAlignment::Top, px(0.));
    list.splice(1..2, 2);
    assert_eq!(list.item_count(), 5);
    assert!(!tracy_client::Client::is_running());

    let spans = spans.lock().unwrap();
    assert!(
        spans.iter().any(|(_, name)| *name == "edit-list"),
        "{spans:?}"
    );
    assert!(
        spans.contains(&("open_gpui_sum_tree::cursor", "seek_forward")),
        "{spans:?}"
    );
}
