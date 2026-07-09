use open_gpui_devtools::{
    ProbeId, SnapshotKind, TimelineEventSnapshot, TimelineSnapshot, timeline,
};

#[test]
fn timeline_snapshots_sanitize_events_and_payloads() {
    let snapshot = TimelineSnapshot::new(
        "owner alice@example.com",
        "Frame token=raw-secret",
        [TimelineEventSnapshot::new(
            "event alice@example.com",
            "Layout token=raw-secret",
            "layout",
            7,
        )
        .timestamp_ms(12)
        .duration_ms(4)
        .with_payload(serde_json::json!({
            "owner": "alice@example.com",
            "path": "C:\\Users\\Frank\\timeline.json",
            "token": "token=raw-secret",
        }))],
    );
    let envelope =
        timeline::timeline_snapshot_envelope(ProbeId::new("timeline").unwrap(), &snapshot);
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.kind, SnapshotKind::Timeline);
    assert!(serialized.contains("timeline"));
    assert!(serialized.contains("\"order\":7"));
    assert!(serialized.contains("\"timestamp_ms\":12"));
    assert!(serialized.contains("\"duration_ms\":4"));
    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-secret"), "{serialized}");
    assert!(!serialized.contains("Frank"), "{serialized}");
    assert!(serialized.contains("[redacted"));
}

#[test]
fn timeline_snapshots_bound_event_collections() {
    let snapshot = TimelineSnapshot::with_event_limit(
        "frames",
        "Frames",
        (0..5).map(|order| {
            TimelineEventSnapshot::new(
                format!("event-{order}"),
                format!("Event {order}"),
                "motion",
                order,
            )
        }),
        2,
    );
    let serialized = serde_json::to_string(&snapshot.tree()).unwrap();

    assert_eq!(snapshot.events().len(), 2);
    assert_eq!(snapshot.max_events(), 2);
    assert_eq!(snapshot.omitted_events(), 3);
    assert!(serialized.contains("\"event_count\":2"));
    assert!(serialized.contains("\"omitted_events\":3"));
    assert!(serialized.contains("event-0"));
    assert!(serialized.contains("event-1"));
    assert!(!serialized.contains("event-2"));
}

#[cfg(feature = "motion")]
#[test]
fn motion_adapter_projects_frame_demand_as_timeline() {
    use open_gpui_devtools::motion;
    use open_gpui_motion::{MotionFrameDemand, MotionFrameReason};

    let snapshot = motion::motion_frame_demand_timeline_snapshot(MotionFrameDemand::NeedsFrame(
        MotionFrameReason::UpdateRender,
    ));
    let envelope = snapshot.envelope(ProbeId::new("motion.timeline").unwrap());
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.kind, SnapshotKind::Timeline);
    assert!(serialized.contains("Motion frame demand"));
    assert!(serialized.contains("\"needs_frame\":true"));
    assert!(serialized.contains("update-render"));
    assert!(!serialized.contains("UpdateRender"));
}
