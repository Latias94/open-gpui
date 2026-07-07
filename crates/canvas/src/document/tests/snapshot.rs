use super::*;

#[test]
fn defaults_to_current_format_version() {
    let document = CanvasDocument::default();

    assert_eq!(document.format_version, CANVAS_DOCUMENT_FORMAT_VERSION);
}

#[test]
fn deserializes_missing_format_version_to_current_version() {
    let document: CanvasDocument = serde_json::from_str(
        r#"{
            "nodes": {},
            "edges": {},
            "shapes": {},
            "metadata": {}
        }"#,
    )
    .unwrap();

    assert_eq!(document.format_version, CANVAS_DOCUMENT_FORMAT_VERSION);
}

#[test]
fn snapshot_round_trips_array_records() {
    let document = connected_pair_fixture().build();

    let snapshot = document.to_snapshot();
    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.edges.len(), 1);

    let restored = CanvasDocument::from_snapshot(snapshot).unwrap();
    assert_eq!(restored.nodes.len(), 2);
    assert_eq!(restored.edges.len(), 1);
}

#[test]
fn snapshot_round_trips_record_relations() {
    let mut document = document_fixture()
        .node(CanvasNode::new(
            "child",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        ))
        .shape(CanvasShape::new(
            "group",
            Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0))),
        ))
        .build();
    let child = CanvasRecordId::Node(NodeId::from("child"));
    let group = CanvasRecordId::Shape(ShapeId::from("group"));
    let binding = CanvasRecordBindingRelation::new("binding", child.clone(), group.clone());
    document
        .apply_transaction(CanvasTransaction::new([
            DocumentCommand::SetRecordParent {
                child: child.clone(),
                parent: group.clone(),
            },
            DocumentCommand::AddRecordToGroup {
                group: group.clone(),
                member: child.clone(),
            },
            DocumentCommand::SetRecordBinding(binding.clone()),
        ]))
        .unwrap();

    let restored = CanvasDocument::from_snapshot(document.to_snapshot()).unwrap();

    assert_eq!(restored.relations().parent_of(&child), Some(&group));
    assert_eq!(
        restored
            .relations()
            .members_of(&group)
            .cloned()
            .collect::<Vec<_>>(),
        vec![child.clone()]
    );
    assert_eq!(restored.relations().binding(&binding.id), Some(&binding));
}

#[test]
fn current_snapshot_migration_is_noop() {
    let mut snapshot = CanvasSnapshot::default();
    snapshot.nodes.push(CanvasNode::new(
        "a",
        point(px(0.0), px(0.0)),
        size(px(10.0), px(10.0)),
    ));
    snapshot.metadata.insert("title".into(), "Canvas".into());

    let migrated = snapshot.clone().migrate_to_current().unwrap();

    assert_eq!(migrated, snapshot);
    assert_eq!(migrated.format_version, CANVAS_DOCUMENT_FORMAT_VERSION);
}

#[test]
fn from_snapshot_accepts_current_snapshot_through_migration_boundary() {
    let snapshot = CanvasSnapshot {
        nodes: vec![CanvasNode::new(
            "a",
            point(px(0.0), px(0.0)),
            size(px(10.0), px(10.0)),
        )],
        ..CanvasSnapshot::default()
    };

    let document = CanvasDocument::from_snapshot(snapshot).unwrap();

    assert_eq!(document.format_version, CANVAS_DOCUMENT_FORMAT_VERSION);
    assert_eq!(document.nodes.len(), 1);
}

#[test]
fn rejects_future_snapshot_version() {
    let snapshot = CanvasSnapshot {
        format_version: CANVAS_DOCUMENT_FORMAT_VERSION + 1,
        ..CanvasSnapshot::default()
    };

    assert_eq!(
        CanvasDocument::from_snapshot(snapshot).unwrap_err(),
        DocumentError::UnsupportedFormatVersion {
            expected: CANVAS_DOCUMENT_FORMAT_VERSION,
            found: CANVAS_DOCUMENT_FORMAT_VERSION + 1,
        }
    );
}

#[test]
fn rejects_snapshot_version_below_minimum_supported_version() {
    let snapshot = CanvasSnapshot {
        format_version: CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION - 1,
        ..CanvasSnapshot::default()
    };

    assert_eq!(
        migrate_canvas_snapshot(snapshot).unwrap_err(),
        DocumentError::UnsupportedFormatVersion {
            expected: CANVAS_DOCUMENT_FORMAT_VERSION,
            found: CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION - 1,
        }
    );
}

#[test]
fn snapshot_migration_table_is_monotonic() {
    for migration in CANVAS_SNAPSHOT_MIGRATIONS {
        assert!(migration.from_version < migration.to_version);
    }

    for migrations in CANVAS_SNAPSHOT_MIGRATIONS.windows(2) {
        assert_eq!(migrations[0].to_version, migrations[1].from_version);
    }
}
