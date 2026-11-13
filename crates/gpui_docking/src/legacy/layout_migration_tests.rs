use crate::legacy::layout_migration::{
    CANONICAL_LAYOUT_VERSION, CanonicalLayoutCentralV3, CanonicalLayoutDecodeError,
    CanonicalLayoutFloatingV3, CanonicalLayoutNodeV3, CanonicalLayoutSizeRefV3,
    CanonicalLayoutSpaceV3, CanonicalLayoutV3, LayoutMigrationError, decode_canonical_layout_json,
    migrate_layout_v2, validate_layout_v3,
};
use crate::legacy::model::{DockLayoutCentralRegion, DockLayoutNode, DockLayoutSpace};
use crate::legacy::{
    CanonicalGraphAuthority, DockGroupId, DockItemId, DockLayout, DockLayoutRect, DockSpaceId,
    SplitAxis,
};

fn item(id: &str) -> DockItemId {
    DockItemId::new(id)
}

fn tabs(id: u32, name: &str) -> DockLayoutNode {
    DockLayoutNode::Tabs {
        id,
        items: vec![item(name)],
        selected: Some(item(name)),
    }
}

#[test]
fn v2_three_child_split_migrates_to_deterministic_right_deep_binary_tree() {
    let layout = DockLayout::from_raw_parts(
        vec![DockLayoutSpace {
            id: DockSpaceId::from("main"),
            root: Some(10),
            floatings: Vec::new(),
            central: Some(DockLayoutCentralRegion {
                node: Some(2),
                keep_alive_when_empty: true,
                passthrough_when_empty: false,
            }),
        }],
        vec![
            tabs(1, "A"),
            tabs(2, "B"),
            tabs(3, "C"),
            DockLayoutNode::Split {
                id: 10,
                axis: SplitAxis::Horizontal,
                children: vec![1, 2, 3],
                fractions: vec![0.2, 0.3, 0.5],
            },
        ],
    );
    let migrated = migrate_layout_v2(&layout).expect("v2 layout should migrate");
    assert_eq!(migrated.layout_version, CANONICAL_LAYOUT_VERSION);
    assert_eq!(migrated.spaces[0].root, Some(4));
    assert_eq!(migrated.spaces[0].central.as_ref().unwrap().node, Some(2));
    let branches = migrated
        .nodes
        .iter()
        .filter_map(|node| match node {
            CanonicalLayoutNodeV3::Branch { ratio, .. } => Some(*ratio),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(branches.len(), 2);
    assert!((branches[0] - 0.375).abs() < 0.001 || (branches[1] - 0.375).abs() < 0.001);
    assert!(migrated.nodes.iter().any(|node| matches!(
        node,
        CanonicalLayoutNodeV3::Group { group_id, .. }
            if group_id == "group:4:main:main/0"
    )));
    validate_layout_v3(&migrated).expect("migrated layout should validate");
}

#[test]
fn v2_root_split_central_resolves_to_remainder_group_leaf() {
    let layout = DockLayout::from_raw_parts(
        vec![DockLayoutSpace {
            id: DockSpaceId::from("main"),
            root: Some(10),
            floatings: Vec::new(),
            central: Some(DockLayoutCentralRegion {
                node: Some(10),
                keep_alive_when_empty: true,
                passthrough_when_empty: false,
            }),
        }],
        vec![
            tabs(1, "A"),
            tabs(2, "B"),
            tabs(3, "C"),
            DockLayoutNode::Split {
                id: 10,
                axis: SplitAxis::Horizontal,
                children: vec![1, 2, 3],
                fractions: vec![0.2, 0.3, 0.5],
            },
        ],
    );

    let migrated = migrate_layout_v2(&layout).expect("split central should migrate");
    let central = migrated.spaces[0]
        .central
        .as_ref()
        .and_then(|central| central.node)
        .expect("central policy should resolve to a group leaf");

    assert!(migrated.nodes.iter().any(|node| matches!(
        node,
        CanonicalLayoutNodeV3::Group {
            id,
            group_id,
            items,
            ..
        } if *id == central
            && group_id == "group:4:main:main/2"
            && items.len() == 1
            && items[0] == item("C")
    )));
    validate_layout_v3(&migrated).expect("resolved central leaf should validate");
}

#[test]
fn v2_zero_share_suffix_and_floating_root_roundtrip_without_loss() {
    let children = (1..=8).collect::<Vec<_>>();
    let items = (1..=9)
        .map(|id| item(&format!("P{id}")))
        .collect::<Vec<_>>();
    let nodes = (1..=9)
        .zip(items.iter())
        .map(|(id, item)| DockLayoutNode::Tabs {
            id,
            items: vec![item.clone()],
            selected: Some(item.clone()),
        })
        .chain(std::iter::once(DockLayoutNode::Split {
            id: 100,
            axis: SplitAxis::Vertical,
            children: children.clone(),
            fractions: vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        }))
        .collect();
    let layout = DockLayout::from_raw_parts(
        vec![DockLayoutSpace {
            id: DockSpaceId::from("main"),
            root: Some(100),
            floatings: vec![crate::legacy::model::DockLayoutFloatingContainer {
                root: 9,
                bounds: DockLayoutRect {
                    x: 1.0,
                    y: 2.0,
                    width: 300.0,
                    height: 200.0,
                },
            }],
            central: None,
        }],
        nodes,
    );
    let migrated = migrate_layout_v2(&layout).expect("zero-share layout should migrate");
    assert_eq!(migrated.spaces[0].floating_roots[0].id, "floating:4:main:0");
    assert!(migrated.nodes.iter().any(|node| matches!(
        node,
        CanonicalLayoutNodeV3::Branch { ratio, .. } if *ratio == 1.0
    )));
    let json = serde_json::to_string(&migrated).unwrap();
    let decoded: CanonicalLayoutV3 = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, migrated);
}

#[test]
fn v3_import_export_preserves_durable_identity_and_drops_runtime_mru() {
    let layout = CanonicalLayoutV3 {
        layout_version: CANONICAL_LAYOUT_VERSION,
        spaces: vec![CanonicalLayoutSpaceV3 {
            id: DockSpaceId::from("main"),
            root: Some(10),
            floating_roots: vec![CanonicalLayoutFloatingV3 {
                id: "floating:4:main:tools".into(),
                root: 13,
                bounds: DockLayoutRect {
                    x: 10.0,
                    y: 20.0,
                    width: 420.0,
                    height: 280.0,
                },
                z_order: 7,
            }],
            central: Some(CanonicalLayoutCentralV3 {
                node: Some(11),
                keep_alive_when_empty: true,
                passthrough_when_empty: false,
            }),
        }],
        nodes: vec![
            CanonicalLayoutNodeV3::Branch {
                id: 10,
                axis: SplitAxis::Horizontal,
                first: 11,
                second: 12,
                ratio: 0.7,
                size_ref: Some(CanonicalLayoutSizeRefV3 {
                    width: 1_200.0,
                    height: 800.0,
                }),
            },
            CanonicalLayoutNodeV3::Group {
                id: 11,
                group_id: "editor".into(),
                items: vec![item("A"), item("B")],
                selected: item("B"),
                size_ref: Some(CanonicalLayoutSizeRefV3 {
                    width: 840.0,
                    height: 800.0,
                }),
            },
            CanonicalLayoutNodeV3::Group {
                id: 12,
                group_id: "inspector".into(),
                items: vec![item("C")],
                selected: item("C"),
                size_ref: None,
            },
            CanonicalLayoutNodeV3::Group {
                id: 13,
                group_id: "tools".into(),
                items: vec![item("D")],
                selected: item("D"),
                size_ref: None,
            },
        ],
    };

    let mut first_authority = CanonicalGraphAuthority::new();
    let mut graph = layout
        .import_graph(&mut first_authority)
        .expect("valid v3 should publish through assembly");
    graph
        .group_mut_for_test(&DockGroupId::from("editor"))
        .unwrap()
        .mru = vec![item("A")];
    let exported = CanonicalLayoutV3::from_graph(&graph).unwrap();
    let mut second_authority = CanonicalGraphAuthority::new();
    let restored = exported.import_graph(&mut second_authority).unwrap();
    let reexported = CanonicalLayoutV3::from_graph(&restored).unwrap();

    assert_eq!(reexported, exported);
    assert_eq!(
        restored.group(&DockGroupId::from("editor")).unwrap().items,
        vec![item("A"), item("B")]
    );
    assert_eq!(
        restored
            .group(&DockGroupId::from("editor"))
            .unwrap()
            .selected,
        item("B")
    );
    assert!(
        restored
            .group(&DockGroupId::from("editor"))
            .unwrap()
            .mru
            .is_empty()
    );
    assert_eq!(
        reexported.spaces[0].floating_roots[0].id,
        "floating:4:main:tools"
    );
    assert_eq!(reexported.spaces[0].floating_roots[0].z_order, 7);
}

#[test]
fn v3_validation_rejects_unsupported_versions_duplicate_ids_and_invalid_central_nodes() {
    let mut unsupported = simple_v3();
    unsupported.layout_version = 99;
    assert_eq!(
        validate_layout_v3(&unsupported),
        Err(LayoutMigrationError::UnsupportedVersion { actual: 99 })
    );

    let mut duplicate_space = simple_v3();
    duplicate_space
        .spaces
        .push(duplicate_space.spaces[0].clone());
    assert!(matches!(
        validate_layout_v3(&duplicate_space),
        Err(LayoutMigrationError::DuplicateSpace { .. })
    ));

    let mut empty_space = simple_v3();
    empty_space.spaces[0].id = DockSpaceId::from("");
    assert_eq!(
        validate_layout_v3(&empty_space),
        Err(LayoutMigrationError::EmptySpaceId)
    );

    let mut duplicate_group = simple_v3();
    duplicate_group.nodes.push(CanonicalLayoutNodeV3::Group {
        id: 2,
        group_id: "editor".into(),
        items: vec![item("B")],
        selected: item("B"),
        size_ref: None,
    });
    duplicate_group.spaces[0].floating_roots = vec![CanonicalLayoutFloatingV3 {
        id: "floating:4:main:0".into(),
        root: 2,
        bounds: DockLayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        z_order: 0,
    }];
    assert!(matches!(
        validate_layout_v3(&duplicate_group),
        Err(LayoutMigrationError::DuplicateGroupId { .. })
    ));

    let mut branch_central = simple_v3();
    branch_central.nodes = vec![
        CanonicalLayoutNodeV3::Branch {
            id: 10,
            axis: SplitAxis::Horizontal,
            first: 1,
            second: 2,
            ratio: 0.5,
            size_ref: None,
        },
        branch_central.nodes[0].clone(),
        CanonicalLayoutNodeV3::Group {
            id: 2,
            group_id: "inspector".into(),
            items: vec![item("B")],
            selected: item("B"),
            size_ref: None,
        },
    ];
    branch_central.spaces[0].root = Some(10);
    branch_central.spaces[0].central.as_mut().unwrap().node = Some(10);
    assert!(matches!(
        validate_layout_v3(&branch_central),
        Err(LayoutMigrationError::CentralNodeNotGroup { node: 10, .. })
    ));
}

#[test]
fn v3_validation_rejects_floating_central_and_invalid_floating_bounds() {
    let mut floating_central = simple_v3();
    floating_central.nodes.push(CanonicalLayoutNodeV3::Group {
        id: 2,
        group_id: "tools".into(),
        items: vec![item("B")],
        selected: item("B"),
        size_ref: None,
    });
    floating_central.spaces[0].floating_roots = vec![CanonicalLayoutFloatingV3 {
        id: "floating:4:main:0".into(),
        root: 2,
        bounds: DockLayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        z_order: 0,
    }];
    floating_central.spaces[0].central.as_mut().unwrap().node = Some(2);
    assert!(matches!(
        validate_layout_v3(&floating_central),
        Err(LayoutMigrationError::CentralOutsideSpace { node: 2, .. })
    ));

    let mut invalid_bounds = simple_v3();
    invalid_bounds.spaces[0].floating_roots = vec![CanonicalLayoutFloatingV3 {
        id: "floating:main:0".into(),
        root: 1,
        bounds: DockLayoutRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 100.0,
        },
        z_order: 0,
    }];
    invalid_bounds.spaces[0].root = None;
    invalid_bounds.spaces[0].central = None;
    assert!(matches!(
        validate_layout_v3(&invalid_bounds),
        Err(LayoutMigrationError::InvalidFloatingBounds { .. })
    ));

    let mut duplicate_z_order = simple_v3();
    duplicate_z_order.nodes.extend([
        CanonicalLayoutNodeV3::Group {
            id: 2,
            group_id: "tools".into(),
            items: vec![item("B")],
            selected: item("B"),
            size_ref: None,
        },
        CanonicalLayoutNodeV3::Group {
            id: 3,
            group_id: "console".into(),
            items: vec![item("C")],
            selected: item("C"),
            size_ref: None,
        },
    ]);
    duplicate_z_order.spaces[0].floating_roots = vec![
        CanonicalLayoutFloatingV3 {
            id: "floating:4:main:0".into(),
            root: 2,
            bounds: DockLayoutRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z_order: 1,
        },
        CanonicalLayoutFloatingV3 {
            id: "floating:4:main:1".into(),
            root: 3,
            bounds: DockLayoutRect {
                x: 100.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z_order: 1,
        },
    ];
    assert!(matches!(
        validate_layout_v3(&duplicate_z_order),
        Err(LayoutMigrationError::DuplicateFloatingZOrder { z_order: 1, .. })
    ));
}

#[test]
fn v2_all_zero_split_is_rejected_but_zero_suffix_is_canonical() {
    let all_zero = DockLayout::from_raw_parts(
        vec![DockLayoutSpace {
            id: DockSpaceId::from("main"),
            root: Some(10),
            floatings: Vec::new(),
            central: None,
        }],
        vec![
            tabs(1, "A"),
            tabs(2, "B"),
            DockLayoutNode::Split {
                id: 10,
                axis: SplitAxis::Horizontal,
                children: vec![1, 2],
                fractions: vec![0.0, 0.0],
            },
        ],
    );
    assert_eq!(
        migrate_layout_v2(&all_zero),
        Err(LayoutMigrationError::AllZeroSplit { id: 10 })
    );

    let zero_suffix = DockLayout::from_raw_parts(
        vec![DockLayoutSpace {
            id: DockSpaceId::from("main"),
            root: Some(10),
            floatings: Vec::new(),
            central: None,
        }],
        vec![
            tabs(1, "A"),
            tabs(2, "B"),
            tabs(3, "C"),
            DockLayoutNode::Split {
                id: 10,
                axis: SplitAxis::Horizontal,
                children: vec![1, 2, 3],
                fractions: vec![1.0, 0.0, 0.0],
            },
        ],
    );
    let migrated = migrate_layout_v2(&zero_suffix).unwrap();
    assert!(migrated.nodes.iter().any(|node| matches!(
        node,
        CanonicalLayoutNodeV3::Branch { id, ratio, .. }
            if *id != 10 && *ratio == 0.5
    )));
}

#[test]
fn v2_durable_ids_depend_on_space_scope_and_path_not_legacy_node_ids_or_order() {
    fn layout(root: u32, first: u32, second: u32, floating: u32, reorder: bool) -> DockLayout {
        let mut nodes = vec![
            tabs(first, "A"),
            tabs(second, "B"),
            tabs(floating, "F"),
            DockLayoutNode::Split {
                id: root,
                axis: SplitAxis::Horizontal,
                children: vec![first, second],
                fractions: vec![1.0, 1.0],
            },
        ];
        if reorder {
            nodes.reverse();
        }
        DockLayout::from_raw_parts(
            vec![DockLayoutSpace {
                id: DockSpaceId::from("main"),
                root: Some(root),
                floatings: vec![crate::legacy::model::DockLayoutFloatingContainer {
                    root: floating,
                    bounds: DockLayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                }],
                central: None,
            }],
            nodes,
        )
    }

    fn durable_ids(layout: &CanonicalLayoutV3) -> Vec<(String, String)> {
        let mut ids = layout
            .nodes
            .iter()
            .filter_map(|node| match node {
                CanonicalLayoutNodeV3::Group {
                    group_id, items, ..
                } => Some((items[0].as_str().to_owned(), group_id.clone())),
                CanonicalLayoutNodeV3::Branch { .. } => None,
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    let first = migrate_layout_v2(&layout(10, 1, 2, 3, false)).unwrap();
    let second = migrate_layout_v2(&layout(900, 500, 400, 600, true)).unwrap();
    assert_eq!(durable_ids(&first), durable_ids(&second));
    assert_eq!(
        first.spaces[0].floating_roots[0].id,
        second.spaces[0].floating_roots[0].id
    );
    assert_eq!(first.spaces[0].floating_roots[0].z_order, 0);
}

#[test]
fn wire_decoder_dispatches_by_version_before_decoding_the_schema() {
    let legacy = DockLayout::from_raw_parts(
        vec![DockLayoutSpace {
            id: DockSpaceId::from("main"),
            root: Some(1),
            floatings: Vec::new(),
            central: None,
        }],
        vec![tabs(1, "A")],
    );
    let decoded_v2 =
        decode_canonical_layout_json(&serde_json::to_string(&legacy).unwrap()).unwrap();
    assert_eq!(decoded_v2.layout_version, CANONICAL_LAYOUT_VERSION);

    let v3 = simple_v3();
    let decoded_v3 = decode_canonical_layout_json(&serde_json::to_string(&v3).unwrap()).unwrap();
    assert_eq!(decoded_v3, v3);

    assert!(matches!(
        decode_canonical_layout_json(r#"{"layout_version":99,"spaces":[],"nodes":[]}"#),
        Err(CanonicalLayoutDecodeError::UnsupportedVersion { actual: 99 })
    ));
    assert!(matches!(
        decode_canonical_layout_json(r#"{"spaces":[],"nodes":[]}"#),
        Err(CanonicalLayoutDecodeError::Json(_))
    ));
    assert!(matches!(
        decode_canonical_layout_json(
            r#"{"layout_version":3,"spaces":[],"nodes":[],"unexpected":true}"#
        ),
        Err(CanonicalLayoutDecodeError::Json(_))
    ));
}

fn simple_v3() -> CanonicalLayoutV3 {
    CanonicalLayoutV3 {
        layout_version: CANONICAL_LAYOUT_VERSION,
        spaces: vec![CanonicalLayoutSpaceV3 {
            id: DockSpaceId::from("main"),
            root: Some(1),
            floating_roots: Vec::new(),
            central: Some(CanonicalLayoutCentralV3 {
                node: Some(1),
                keep_alive_when_empty: true,
                passthrough_when_empty: false,
            }),
        }],
        nodes: vec![CanonicalLayoutNodeV3::Group {
            id: 1,
            group_id: "editor".into(),
            items: vec![item("A")],
            selected: item("A"),
            size_ref: None,
        }],
    }
}
