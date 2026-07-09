use open_gpui_devtools::{
    LayoutBoundsSnapshot, LayoutNodeSnapshot, LayoutPointSnapshot, LayoutSizeSnapshot,
    LayoutSnapshot, ProbeId, SnapshotKind, layout,
};

#[test]
fn layout_snapshots_sanitize_nodes_and_payloads() {
    let snapshot = LayoutSnapshot::new(
        "owner alice@example.com",
        "Layout token=raw-secret",
        [LayoutNodeSnapshot::new(
            "node alice@example.com",
            "Root C:\\Users\\Frank\\layout.json",
        )
        .bounds(LayoutBoundsSnapshot::new(
            LayoutPointSnapshot::new(1.0, 2.0),
            LayoutSizeSnapshot::new(300.0, 200.0),
        ))
        .content_size(LayoutSizeSnapshot::new(600.0, 500.0))
        .scroll_offset(LayoutPointSnapshot::new(4.0, 8.0))
        .max_scroll_offset(LayoutPointSnapshot::new(40.0, 80.0))
        .with_payload(serde_json::json!({
            "owner": "alice@example.com",
            "path": "C:\\Users\\Frank\\layout.json",
            "token": "token=raw-secret",
        }))],
    );
    let envelope = layout::layout_snapshot_envelope(ProbeId::new("layout").unwrap(), &snapshot);
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.kind, SnapshotKind::Layout);
    assert!(serialized.contains("\"width\":300.0"));
    assert!(serialized.contains("\"height\":200.0"));
    assert!(serialized.contains("\"max_scroll_offset\""));
    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-secret"), "{serialized}");
    assert!(!serialized.contains("Frank"), "{serialized}");
    assert!(serialized.contains("[redacted"));
}

#[cfg(feature = "gpui")]
#[test]
fn gpui_adapter_projects_scroll_viewport_as_layout() {
    use open_gpui::{ScrollViewportChangeSource, ScrollViewportSnapshot, bounds, point, px, size};
    use open_gpui_devtools::gpui;

    let viewport = ScrollViewportSnapshot::new(
        7,
        ScrollViewportChangeSource::InitialLayout,
        bounds(point(px(1.0), px(2.0)), size(px(300.0), px(200.0))),
        point(px(4.0), px(8.0)),
        point(px(40.0), px(80.0)),
        size(px(600.0), px(500.0)),
    );
    let snapshot = gpui::scroll_viewport_layout_snapshot(viewport);
    let envelope = snapshot.envelope(ProbeId::new("layout.scroll").unwrap());
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.kind, SnapshotKind::Layout);
    assert!(serialized.contains("Scroll viewport layout"));
    assert!(serialized.contains("initial-layout"));
    assert!(serialized.contains("\"generation\":7"));
    assert!(serialized.contains("\"width\":300.0"));
    assert!(serialized.contains("\"x\":4.0"));
    assert!(!serialized.contains("InitialLayout"));
}
