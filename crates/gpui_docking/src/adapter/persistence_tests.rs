use super::*;
use dockspace::model::{DockAnchor, DockPlacement};
use dockspace::runtime::{DockspacePersistenceErrorKind, SurfaceUnavailableReason};
use serde_json::{Value, json};
use std::cell::Cell;

const DOCUMENT: DockspaceDocumentId = DockspaceDocumentId::from_bytes([0x71; 16]);
const KEYS: [&str; 7] = [
    "inspector",
    "scene:asset:left",
    "scene:asset:draft",
    "hierarchy",
    "console",
    "scene:asset:right",
    "closed:history",
];

const LAYOUT_V2: &str = r#"{
  "layout_version": 2,
  "spaces": [
    {"id":"tools","root":5},
    {"id":"main","root":10,
     "floatings":[{"root":4,"bounds":{"x":10.25,"y":20.5,"width":449.5,"height":240.25}}],
     "central":{"node":2,"keep_alive_when_empty":true,"passthrough_when_empty":true}}
  ],
  "nodes": [
    {"kind":"tabs","id":1,"items":["inspector"],"selected":"inspector"},
    {"kind":"tabs","id":2,"items":["scene:asset:left","scene:asset:draft"],"selected":"scene:asset:draft"},
    {"kind":"tabs","id":3,"items":["hierarchy"],"selected":"hierarchy"},
    {"kind":"tabs","id":4,"items":["console"],"selected":"console"},
    {"kind":"tabs","id":5,"items":["scene:asset:right"],"selected":"scene:asset:right"},
    {"kind":"split","id":10,"axis":"Horizontal","children":[1,2,3],"fractions":[0.2,0.0,0.3]}
  ]
}"#;

const LAYOUT_V3: &str = r#"{
  "layout_version":3,
  "spaces":[{"id":"main","root":10,
    "floating_roots":[
      {"id":"floating:console","root":4,"bounds":{"x":20.25,"y":-10.5,"width":400.5,"height":220.25},"z_order":20},
      {"id":"floating:tools","root":5,"bounds":{"x":30.5,"y":15.25,"width":310.25,"height":180.5},"z_order":-2}
    ],
    "central":{"node":3,"keep_alive_when_empty":false,"passthrough_when_empty":true}
  }],
  "nodes":[
    {"kind":"branch","id":10,"axis":"Horizontal","first":1,"second":11,"ratio":0.2,"size_ref":{"width":1000.0,"height":600.0}},
    {"kind":"branch","id":11,"axis":"Horizontal","first":2,"second":3,"ratio":0.375},
    {"kind":"group","id":1,"group_id":"group:inspector","items":["inspector"],"selected":"inspector","size_ref":{"width":200.0,"height":600.0}},
    {"kind":"group","id":2,"group_id":"group:scene","items":["scene:asset:left","scene:asset:draft"],"selected":"scene:asset:left","size_ref":{"width":300.0,"height":600.0}},
    {"kind":"group","id":3,"group_id":"group:hierarchy","items":["hierarchy"],"selected":"hierarchy","size_ref":{"width":500.0,"height":600.0}},
    {"kind":"group","id":4,"group_id":"group:console","items":["console"],"selected":"console"},
    {"kind":"group","id":5,"group_id":"group:tools","items":["scene:asset:right"],"selected":"scene:asset:right"}
  ]
}"#;

fn layout_value(source: &str) -> Value {
    serde_json::from_str(source).expect("fixture must be valid JSON")
}

fn snapshot_value() -> Value {
    json!({
        "revision": 42,
        "layout": layout_value(LAYOUT_V2),
        "viewport_placement": {
            "placement_version": 1,
            "viewports": [
                {
                    "space": "main",
                    "display_id": 9,
                    "window_bounds": {
                        "state": "maximized",
                        "bounds": {"x": -1450.25, "y": -20.5, "width": 1440.5, "height": 900.25}
                    },
                    "host_bounds": {"x": 0.0, "y": 30.0, "width": 1440.5, "height": 870.25}
                },
                {"space": "tools"}
            ]
        }
    })
}

fn migrate_layout(value: &Value) -> Result<RestoredDockWorkspace, DockPersistenceError> {
    migrate_legacy_layout_json(
        &serde_json::to_vec(value).expect("fixture must encode"),
        DOCUMENT,
        KEYS,
        LegacyTopologyMigration::RelativeWeights,
        DockPolicy::default(),
    )
}

fn migrate_snapshot(value: &Value) -> Result<RestoredDockWorkspace, DockPersistenceError> {
    migrate_legacy_surface_snapshot_json(
        &serde_json::to_vec(value).expect("fixture must encode"),
        DOCUMENT,
        KEYS,
        LegacyTopologyMigration::RelativeWeights,
        DockPolicy::default(),
    )
}

fn recognize(document: DockspaceDocumentId, key: &str) -> bool {
    document == DOCUMENT && KEYS.contains(&key)
}

fn pane(session: &DockspaceSession, key: &str) -> ItemId {
    session
        .item_id_for_external_key(key)
        .expect("pane key must be retained")
}

fn main_split(session: &DockspaceSession) -> dockspace::model::DockspaceSplitView<'_> {
    session
        .view()
        .surface(SurfaceId::new(1))
        .expect("main surface")
        .main_root()
        .expect("main root")
        .content()
        .expect("root content")
        .split()
        .expect("canonical split")
}

fn assert_weights(actual: impl IntoIterator<Item = f32>, expected: &[f32]) {
    let actual = actual.into_iter().collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 0.000_001,
            "actual {actual}, expected {expected}"
        );
    }
}

#[test]
fn actual_v2_surface_snapshot_preserves_multiview_order_selection_and_logical_hints() {
    let restored = migrate_snapshot(&snapshot_value()).expect("real v2 schema must migrate");
    assert_eq!(restored.session.document_id(), Some(DOCUMENT));
    assert_eq!(
        restored
            .host
            .surfaces
            .iter()
            .map(|binding| binding.space_key.as_str())
            .collect::<Vec<_>>(),
        ["main", "tools"]
    );
    let left = pane(&restored.session, "scene:asset:left");
    let right = pane(&restored.session, "scene:asset:right");
    let draft = pane(&restored.session, "scene:asset:draft");
    assert_ne!(
        left, right,
        "two View occurrences of one document need distinct ItemIds"
    );
    assert_eq!(
        restored
            .session
            .view()
            .item(left)
            .expect("left View")
            .surface(),
        SurfaceId::new(1)
    );
    assert_eq!(
        restored
            .session
            .view()
            .item(right)
            .expect("right View")
            .surface(),
        SurfaceId::new(2)
    );
    assert_eq!(
        restored
            .session
            .view()
            .item(left)
            .expect("left tabs")
            .tabs()
            .items(),
        [left, draft]
    );
    assert!(
        restored
            .session
            .view()
            .item(draft)
            .expect("selected draft")
            .is_selected()
    );
    assert!(
        restored
            .session
            .view()
            .item(left)
            .expect("central View")
            .tabs()
            .is_central()
    );
    assert_weights(main_split(&restored.session).weights(), &[0.2, 0.5, 0.3]);
    let floating = restored
        .session
        .view()
        .surface(SurfaceId::new(1))
        .expect("main surface")
        .contained()
        .next()
        .expect("console floating");
    assert_eq!(
        floating.rect(),
        LogicalRect::new(10.25, 20.5, 449.5, 240.25).expect("valid rect")
    );
    let main = &restored.host.surfaces[0];
    let placement = main.logical_placement.as_ref().expect("logical placement");
    assert_eq!(placement.display_id, Some(9));
    let window = placement.window_bounds.expect("window hint");
    assert_eq!(window.bounds.x, -1450.25);
    assert_eq!(window.state, DockWindowPlacementState::Maximized);
    assert_eq!(
        main.central_policy,
        Some(DockCentralPolicyHint {
            keep_alive_when_empty: true,
            passthrough_when_empty: true
        })
    );
    let receipt = restored.host.migration.as_ref().expect("migration receipt");
    assert_eq!(receipt.snapshot_revision, Some(42));
    assert_eq!(
        receipt
            .source_nodes
            .iter()
            .map(|node| node.source_node)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 10]
    );
    assert_eq!(receipt.floating_identities[0].source_ordinal, 0);
    assert_eq!(receipt.floating_identities[0].source_id, None);
    assert!(
        receipt
            .differences
            .contains(&DockLegacyContractDifference::LogicalPlacementRequiresHost)
    );
    assert_eq!(
        restored.session.surface_presentation_preferences().count(),
        0,
        "logical hints cannot manufacture native placement facts"
    );
}

#[test]
fn binary_v3_becomes_canonical_nary_and_records_exact_group_floating_and_size_identity() {
    let restored = migrate_layout(&layout_value(LAYOUT_V3)).expect("binary fixture must migrate");
    let split = main_split(&restored.session);
    assert_eq!(split.child_count(), 3);
    assert_weights(split.weights(), &[0.2, 0.3, 0.5]);
    let receipt = restored.host.migration.as_ref().expect("migration receipt");
    assert_eq!(receipt.layout_version, 3);
    assert_eq!(receipt.snapshot_revision, None);
    let scene = receipt
        .source_nodes
        .iter()
        .find(|node| node.source_node == 2)
        .expect("source identity");
    assert_eq!(scene.group_id.as_deref(), Some("group:scene"));
    assert_eq!(
        scene.size_reference,
        Some(DockLegacySizeReference {
            width: 300.0,
            height: 600.0
        })
    );
    assert_eq!(receipt.source_nodes.len(), 7);
    assert_eq!(
        receipt
            .floating_identities
            .iter()
            .map(|floating| floating.source_id.as_deref().expect("v3 identity"))
            .collect::<Vec<_>>(),
        ["floating:tools", "floating:console"]
    );
    assert_eq!(receipt.floating_identities[0].source_z_order, Some(-2));
    assert_eq!(receipt.floating_identities[0].source_ordinal, 1);
    for identity in &receipt.floating_identities {
        let floating = restored
            .session
            .view()
            .contained(identity.initial_floating)
            .expect("initial identity crosswalk");
        assert_eq!(
            floating.root().expect("floating root").id(),
            identity.initial_root
        );
    }
    assert!(
        receipt
            .differences
            .contains(&DockLegacyContractDifference::RelativeGeometry)
    );
    assert!(
        receipt
            .differences
            .contains(&DockLegacyContractDifference::GroupIdentityIsProvenance)
    );
    assert_eq!(
        restored.host.surfaces[0].central_policy,
        Some(DockCentralPolicyHint {
            keep_alive_when_empty: false,
            passthrough_when_empty: true
        })
    );
}

#[test]
fn expressible_child_size_reference_ratio_overrides_the_v3_fallback_ratio() {
    let source = json!({
        "layout_version": 3,
        "spaces": [{"id":"main","root":10}],
        "nodes": [
            {"kind":"branch","id":10,"axis":"Horizontal","first":1,"second":2,"ratio":0.9},
            {"kind":"group","id":1,"group_id":"left","items":["inspector"],"selected":"inspector","size_ref":{"width":100.0,"height":800.0}},
            {"kind":"group","id":2,"group_id":"right","items":["hierarchy"],"selected":"hierarchy","size_ref":{"width":300.0,"height":200.0}}
        ]
    });
    let restored = migrate_layout(&source).expect("paired size refs must migrate");
    assert_weights(main_split(&restored.session).weights(), &[0.25, 0.75]);
    let receipt = restored.host.migration.as_ref().expect("size provenance");
    assert_eq!(
        receipt.source_nodes[0]
            .size_reference
            .expect("exact ref")
            .height,
        800.0
    );
}

#[test]
fn document_roundtrip_retains_closed_keys_and_only_open_occurrences_need_factories() {
    let mut migrated = migrate_snapshot(&snapshot_value()).expect("fixture must migrate");
    let expected = KEYS
        .iter()
        .map(|key| (*key, pane(&migrated.session, key)))
        .collect::<BTreeMap<_, _>>();
    let expected_host = migrated.host.clone();
    let document = DockWorkspaceDocument::capture(&mut migrated.session, migrated.host)
        .expect("atomic capture");
    let bytes = document.to_json().expect("envelope encoding");
    let mut restored =
        DockWorkspaceDocument::restore_json(&bytes, DockPolicy::default(), recognize)
            .expect("document roundtrip");
    assert_eq!(restored.host, expected_host);
    for (key, item) in &expected {
        assert_eq!(pane(&restored.session, key), *item);
        assert_eq!(restored.session.external_key_for_item(*item), Some(*key));
    }
    let bindings = open_pane_bindings(&restored.session).expect("complete factory plan");
    assert_eq!(bindings.len(), KEYS.len() - 1);
    assert!(
        !bindings
            .iter()
            .any(|binding| binding.pane_key == "closed:history")
    );
    assert_eq!(
        bindings
            .iter()
            .map(|binding| binding.item)
            .collect::<BTreeSet<_>>()
            .len(),
        bindings.len()
    );
    let closed = pane(&restored.session, "closed:history");
    assert!(restored.session.view().item(closed).is_none());
    let newly_registered = restored
        .session
        .ensure_external_item("new:after-restore")
        .expect("append-only registration");
    assert!(
        expected
            .values()
            .all(|item| item.get() < newly_registered.get())
    );
    let anchor = pane(&restored.session, "inspector");
    let mut frame = restored.session.begin_host_frame().expect("reopen frame");
    frame
        .open_item_current(closed, DockPlacement::Center(DockAnchor::Item(anchor)))
        .expect("reopen historical identity");
    frame
        .complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)
        .expect("complete logical frame");
    frame.commit().expect("atomic reopen");
    assert!(restored.session.view().item(closed).is_some());
    assert_eq!(pane(&restored.session, "closed:history"), closed);
}

#[test]
fn v3_provenance_and_non_equivalence_receipt_survive_the_new_document_roundtrip() {
    let mut migrated = migrate_layout(&layout_value(LAYOUT_V3)).expect("binary migration");
    let expected = migrated.host.clone();
    let document =
        DockWorkspaceDocument::capture(&mut migrated.session, migrated.host).expect("capture");
    let restored = DockWorkspaceDocument::restore_json(
        &document.to_json().expect("encode"),
        DockPolicy::default(),
        recognize,
    )
    .expect("restore");
    assert_eq!(restored.host, expected);
    assert_weights(main_split(&restored.session).weights(), &[0.2, 0.3, 0.5]);
}

#[test]
fn a_missing_snapshot_revision_defaults_to_zero_as_in_the_actual_old_schema() {
    let mut snapshot = snapshot_value();
    snapshot
        .as_object_mut()
        .expect("snapshot object")
        .remove("revision");
    let restored = migrate_snapshot(&snapshot).expect("pre-revision snapshot");
    assert_eq!(
        restored
            .host
            .migration
            .expect("migration receipt")
            .snapshot_revision,
        Some(0)
    );
}

#[test]
fn empty_spaces_and_rootless_floating_spaces_remain_addressable() {
    let source = json!({
        "layout_version":2,
        "spaces":[{"id":"empty"},{"id":"floating","floatings":[{"root":1,"bounds":{"x":0.0,"y":0.0,"width":100.0,"height":100.0}}]}],
        "nodes":[{"kind":"tabs","id":1,"items":["console"],"selected":"console"}]
    });
    let restored = migrate_layout(&source).expect("empty and floating-only spaces");
    let empty = restored
        .session
        .view()
        .surface(SurfaceId::new(1))
        .expect("empty surface");
    assert!(
        empty
            .main_root()
            .expect("central root")
            .central()
            .expect("central leaf")
            .tabs()
            .expect("central tabs")
            .items()
            .is_empty()
    );
    assert!(
        restored
            .session
            .view()
            .surface(SurfaceId::new(2))
            .expect("floating surface")
            .is_rootless()
    );
    assert!(
        restored
            .host
            .migration
            .expect("migration receipt")
            .differences
            .contains(&DockLegacyContractDifference::EmptySpaceUsesCentralRoot)
    );
}

#[test]
fn unknown_and_duplicate_pane_keys_reject_the_entire_migration() {
    let unknown = migrate_legacy_layout_json(
        LAYOUT_V2.as_bytes(),
        DOCUMENT,
        ["inspector"],
        LegacyTopologyMigration::RelativeWeights,
        DockPolicy::default(),
    )
    .expect_err("unknown panes must fail");
    assert!(matches!(
        unknown,
        DockPersistenceError::UnknownPaneKey { .. }
    ));
    let duplicated = migrate_legacy_layout_json(
        LAYOUT_V2.as_bytes(),
        DOCUMENT,
        KEYS.into_iter().chain(["closed:history"]),
        LegacyTopologyMigration::RelativeWeights,
        DockPolicy::default(),
    )
    .expect_err("duplicate closed keys must fail");
    assert!(
        matches!(duplicated, DockPersistenceError::DuplicatePaneKey { key } if key == "closed:history")
    );
    let mut source = layout_value(LAYOUT_V2);
    source["nodes"][4]["items"] = json!(["scene:asset:left"]);
    source["nodes"][4]["selected"] = json!("scene:asset:left");
    assert!(matches!(
        migrate_layout(&source),
        Err(DockPersistenceError::DuplicatePaneKey { .. })
    ));
}

#[test]
fn cycles_shared_children_missing_nodes_orphans_and_bad_selection_are_atomic_errors() {
    let malformed: [(&str, fn(&mut Value)); 7] = [
        ("cycle", |source| {
            source["nodes"][5]["children"][0] = json!(10)
        }),
        ("shared child", |source| {
            source["nodes"][5]["children"][1] = json!(1)
        }),
        ("missing child", |source| {
            source["nodes"][5]["children"][2] = json!(999)
        }),
        ("orphan", |source| {
            source["spaces"][0]
                .as_object_mut()
                .expect("space")
                .remove("root")
                .map(|_| ())
                .unwrap_or(())
        }),
        ("missing selection", |source| {
            source["nodes"][0]["selected"] = Value::Null
        }),
        ("foreign selection", |source| {
            source["nodes"][0]["selected"] = json!("console")
        }),
        ("bad arity", |source| {
            source["nodes"][5]["fractions"] = json!([0.2])
        }),
    ];
    for (name, mutate) in malformed {
        let mut source = layout_value(LAYOUT_V2);
        mutate(&mut source);
        assert!(
            migrate_layout(&source).is_err(),
            "{name} must reject the complete layout"
        );
    }
}

#[test]
fn malformed_placement_cannot_publish_an_otherwise_valid_layout() {
    let malformed: [(&str, fn(&mut Value)); 4] = [
        ("unknown placement space", |source| {
            source["viewport_placement"]["viewports"][0]["space"] = json!("missing")
        }),
        ("duplicate placement", |source| {
            source["viewport_placement"]["viewports"][1]["space"] = json!("main")
        }),
        ("negative geometry", |source| {
            source["viewport_placement"]["viewports"][0]["window_bounds"]["bounds"]["width"] =
                json!(-1.0)
        }),
        ("unsupported placement schema", |source| {
            source["viewport_placement"]["placement_version"] = json!(99)
        }),
    ];
    for (name, mutate) in malformed {
        let mut source = snapshot_value();
        mutate(&mut source);
        assert!(
            migrate_snapshot(&source).is_err(),
            "{name} must reject the entire snapshot"
        );
    }
}

#[test]
fn v3_duplicate_durable_ids_z_order_and_bad_size_refs_are_not_discarded_before_validation() {
    let malformed: [(&str, fn(&mut Value)); 5] = [
        ("group identity", |source| {
            source["nodes"][3]["group_id"] = json!("group:inspector")
        }),
        ("floating identity", |source| {
            source["spaces"][0]["floating_roots"][1]["id"] = json!("floating:console")
        }),
        ("floating z order", |source| {
            source["spaces"][0]["floating_roots"][1]["z_order"] = json!(20)
        }),
        ("negative size reference", |source| {
            source["nodes"][2]["size_ref"]["width"] = json!(-1.0)
        }),
        ("central branch", |source| {
            source["spaces"][0]["central"]["node"] = json!(10)
        }),
    ];
    for (name, mutate) in malformed {
        let mut source = layout_value(LAYOUT_V3);
        mutate(&mut source);
        assert!(migrate_layout(&source).is_err(), "invalid {name} must fail");
    }
}

#[test]
fn zero_share_is_rejected_instead_of_losing_a_key_or_adding_epsilon() {
    let mut source = layout_value(LAYOUT_V2);
    source["nodes"][5]["fractions"] = json!([0.0, 0.0, 0.3]);
    assert!(matches!(
        migrate_layout(&source),
        Err(DockPersistenceError::ZeroShare { node: 10 })
    ));
    let mut source = layout_value(LAYOUT_V3);
    source["nodes"][0]["ratio"] = json!(0.0);
    assert!(matches!(
        migrate_layout(&source),
        Err(DockPersistenceError::ZeroShare { node: 10 })
    ));
    let mut source = layout_value(LAYOUT_V2);
    source["spaces"][1]["floatings"][0]["bounds"]["width"] = json!(0.0);
    assert!(
        migrate_layout(&source).is_err(),
        "a zero-area legacy floating has no core equivalent"
    );
}

#[test]
fn supported_schemas_reject_unknown_fields_duplicate_json_fields_and_bad_numeric_tokens() {
    let malformed = [
        LAYOUT_V2.replacen(
            "\"layout_version\": 2",
            "\"layout_version\": 2, \"unexpected\": true",
            1,
        ),
        LAYOUT_V2.replacen("\"root\":5", "\"root\":5,\"root\":5", 1),
        LAYOUT_V2.replacen(
            "\"selected\":\"inspector\"",
            "\"selected\":\"inspector\",\"native_handle\":7",
            1,
        ),
        LAYOUT_V2.replacen("0.3", "1e999", 1),
    ];
    for source in malformed {
        assert!(
            migrate_legacy_layout_json(
                source.as_bytes(),
                DOCUMENT,
                KEYS,
                LegacyTopologyMigration::RelativeWeights,
                DockPolicy::default()
            )
            .is_err()
        );
    }
    assert!(matches!(
        migrate_layout(&json!({"layout_version":99})),
        Err(DockPersistenceError::UnsupportedLegacyVersion { found: 99 })
    ));
    let unknown = br#"{"format":"open-gpui-dockspace","version":99,"document_json":null}"#;
    assert!(matches!(
        DockWorkspaceDocument::decode_json(unknown),
        Err(DockPersistenceError::UnsupportedEnvelopeVersion { found: 99 })
    ));
}

#[test]
fn closed_key_rejection_and_corrupt_core_documents_do_not_reach_host_factories() {
    let mut active = migrate_snapshot(&snapshot_value()).expect("active workspace");
    let document =
        DockWorkspaceDocument::capture(&mut active.session, active.host.clone()).expect("capture");
    let before_version = active.session.version();
    let before_generation = active.session.next_document_generation();
    let factories_called = Cell::new(0);
    let result = document.restore(DockPolicy::default(), |document, key| {
        recognize(document, key) && key != "closed:history"
    });
    if let Ok(candidate) = &result {
        for _ in open_pane_bindings(&candidate.session).expect("factory plan") {
            factories_called.set(factories_called.get() + 1);
        }
    }
    assert!(
        matches!(result, Err(DockPersistenceError::Core(error)) if error.kind() == DockspacePersistenceErrorKind::IdentityConflict)
    );
    assert_eq!(factories_called.get(), 0);
    let mut envelope: Value =
        serde_json::from_slice(&document.to_json().expect("encode")).expect("envelope");
    let mut inner: Value =
        serde_json::from_str(envelope["document_json"].as_str().expect("opaque JSON"))
            .expect("core payload");
    inner[1]["external_item_keys"]["entries"][0]["item_id"] = json!(9999);
    envelope["document_json"] = json!(serde_json::to_string(&inner).expect("corrupt payload"));
    assert!(
        DockWorkspaceDocument::restore_json(
            &serde_json::to_vec(&envelope).expect("encode corruption"),
            DockPolicy::default(),
            recognize
        )
        .is_err()
    );
    assert_eq!(active.session.version(), before_version);
    assert_eq!(active.session.next_document_generation(), before_generation);
    assert_eq!(
        open_pane_bindings(&active.session)
            .expect("active panes")
            .len(),
        6
    );
}

#[test]
fn envelope_preserves_inner_duplicate_tokens_and_rejects_host_mismatch_or_extra_fields() {
    let mut migrated = migrate_layout(&layout_value(LAYOUT_V2)).expect("migration");
    let document =
        DockWorkspaceDocument::capture(&mut migrated.session, migrated.host).expect("capture");
    let mut envelope: Value =
        serde_json::from_slice(&document.to_json().expect("encode")).expect("envelope");
    let original = envelope["document_json"].as_str().expect("core JSON");
    let duplicated = original.replacen("\"generation\":0", "\"generation\":0,\"generation\":0", 1);
    assert_ne!(original, duplicated, "fixture must duplicate a real field");
    envelope["document_json"] = json!(duplicated);
    assert!(
        DockWorkspaceDocument::restore_json(
            &serde_json::to_vec(&envelope).expect("encode"),
            DockPolicy::default(),
            recognize
        )
        .is_err()
    );
    let mut envelope: Value =
        serde_json::from_slice(&document.to_json().expect("encode")).expect("envelope");
    envelope["host"]["surfaces"][0]["surface"] = json!(987);
    assert!(
        DockWorkspaceDocument::restore_json(
            &serde_json::to_vec(&envelope).expect("encode"),
            DockPolicy::default(),
            recognize
        )
        .is_err()
    );
    envelope["frame_proof"] = json!({"submitted":true});
    assert!(
        DockWorkspaceDocument::decode_json(&serde_json::to_vec(&envelope).expect("encode"))
            .is_err()
    );
}

#[test]
fn invalid_host_metadata_fails_capture_before_advancing_the_core_generation() {
    let mut migrated = migrate_snapshot(&snapshot_value()).expect("migration");
    let generation = migrated.session.next_document_generation();
    migrated.host.surfaces[0]
        .logical_placement
        .as_mut()
        .expect("placement")
        .host_bounds
        .as_mut()
        .expect("host bounds")
        .width = f32::NAN;
    assert!(DockWorkspaceDocument::capture(&mut migrated.session, migrated.host).is_err());
    assert_eq!(migrated.session.next_document_generation(), generation);
}

#[test]
fn serialized_document_has_no_view_native_handle_or_frame_proof_fields() {
    let mut migrated = migrate_snapshot(&snapshot_value()).expect("migration");
    let document =
        DockWorkspaceDocument::capture(&mut migrated.session, migrated.host).expect("capture");
    let text = String::from_utf8(document.to_json().expect("encode")).expect("UTF-8");
    for forbidden in [
        "AnyView",
        "WindowHandle",
        "native_handle",
        "frame_proof",
        "submitted_attempt",
        "FocusHandle",
    ] {
        assert!(
            !text.contains(forbidden),
            "unexpected durable field {forbidden}"
        );
    }
}
