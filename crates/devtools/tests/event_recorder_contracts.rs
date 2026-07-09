use open_gpui_devtools::{
    DevtoolsDomainId, DevtoolsEventKind, DevtoolsEventRecord, DevtoolsEventRecorder,
    DevtoolsTargetId, TimelineSnapshot,
};

#[test]
fn event_recorder_bounds_events_and_reports_omissions() {
    let mut recorder = DevtoolsEventRecorder::with_capacity(2);

    for index in 0..5 {
        recorder.record(DevtoolsEventRecord::new(
            format!("event-{index}"),
            format!("Event {index}"),
            DevtoolsEventKind::Instant,
        ));
    }

    let batch = recorder.snapshot();

    assert_eq!(batch.max_events, 2);
    assert_eq!(batch.omitted_events, 3);
    assert_eq!(
        batch
            .events
            .iter()
            .map(|event| event.sequence())
            .collect::<Vec<_>>(),
        [3, 4]
    );
    assert_eq!(batch.events[0].id(), "event-3");
    assert_eq!(batch.events[1].id(), "event-4");
}

#[test]
fn event_records_sanitize_ids_labels_targets_domains_and_payloads() {
    let mut recorder = DevtoolsEventRecorder::with_capacity(4);
    recorder.record(
        DevtoolsEventRecord::new(
            "event alice@example.com",
            "Label token=raw-secret",
            DevtoolsEventKind::Custom("secret=raw-kind".to_owned()),
        )
        .target_id(DevtoolsTargetId::new("target alice@example.com"))
        .domain_id(DevtoolsDomainId::new("domain alice@example.com"))
        .timestamp_ms(12)
        .duration_ms(4)
        .with_payload(serde_json::json!({
            "owner": "alice@example.com",
            "path": "C:\\Users\\Frank\\event.json",
            "token": "raw-token",
        })),
    );

    let batch = recorder.snapshot();
    let serialized = serde_json::to_string(&batch).unwrap();

    assert_eq!(batch.events[0].sequence(), 0);
    assert_eq!(batch.events[0].timestamp_ms_value(), Some(12));
    assert_eq!(batch.events[0].duration_ms_value(), Some(4));
    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-secret"), "{serialized}");
    assert!(!serialized.contains("raw-kind"), "{serialized}");
    assert!(!serialized.contains("raw-token"), "{serialized}");
    assert!(!serialized.contains("Frank"), "{serialized}");
    assert!(serialized.contains("[redacted"));
}

#[test]
fn timeline_snapshots_project_event_batches() {
    let mut recorder = DevtoolsEventRecorder::with_capacity(3);
    recorder.record(
        DevtoolsEventRecord::new("route", "Viewport route", DevtoolsEventKind::Duration)
            .target_id(DevtoolsTargetId::new("target.viewport"))
            .domain_id(DevtoolsDomainId::new("domain.docking"))
            .timestamp_ms(10)
            .duration_ms(5)
            .with_payload(serde_json::json!({"status": "ready"})),
    );
    let batch = recorder.snapshot();
    let timeline = TimelineSnapshot::from_event_batch("devtools-events", "DevTools events", &batch);
    let serialized = serde_json::to_string(&timeline.tree()).unwrap();

    assert_eq!(timeline.events().len(), 1);
    assert_eq!(timeline.events()[0].order, 0);
    assert_eq!(timeline.events()[0].timestamp_ms, Some(10));
    assert_eq!(timeline.events()[0].duration_ms, Some(5));
    assert!(serialized.contains("Viewport route"));
    assert!(serialized.contains("\"retained_event_count\":1"));
    assert!(serialized.contains("\"target_id\":\"target.viewport\""));
    assert!(serialized.contains("\"domain_id\":\"domain.docking\""));
    assert!(serialized.contains("\"status\":\"ready\""));
}
