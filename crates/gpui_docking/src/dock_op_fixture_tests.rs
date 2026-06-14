use crate::graph_test_support::{main_space as space, root_tabs_graph};
use crate::*;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
struct FixtureSuite {
    schema_version: u32,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    initial: FixtureInitial,
    items: Vec<String>,
    steps: Vec<FixtureStep>,
    expect: FixtureExpect,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureInitial {
    RootTabs,
}

#[derive(Debug, Deserialize)]
struct FixtureExpect {
    item_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FixtureStep {
    MoveItem {
        item: String,
        target_item: String,
        zone: FixtureZone,
    },
    FloatItem {
        item: String,
        bounds: [f32; 4],
    },
    MergeFirstFloating {
        target_item: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixtureZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

impl From<FixtureZone> for DropZone {
    fn from(zone: FixtureZone) -> Self {
        match zone {
            FixtureZone::Center => DropZone::Center,
            FixtureZone::Left => DropZone::Left,
            FixtureZone::Right => DropZone::Right,
            FixtureZone::Top => DropZone::Top,
            FixtureZone::Bottom => DropZone::Bottom,
        }
    }
}

#[test]
fn dock_op_sequence_fixtures_hold_canonical_invariants() {
    let raw = include_str!("fixtures/dock_op_sequences_v1.json");
    let suite: FixtureSuite =
        serde_json::from_str(raw).expect("dock op fixture suite should parse");
    assert_eq!(suite.schema_version, 1);

    for case in suite.cases {
        let FixtureInitial::RootTabs = case.initial;
        let item_refs: Vec<&str> = case.items.iter().map(String::as_str).collect();
        let (mut graph, _root) = root_tabs_graph(&item_refs);

        for step in case.steps {
            apply_fixture_step(&mut graph, step);
            graph.assert_canonical_space(&space());
            assert_unique_items(&graph, &case.id);
        }

        assert_eq!(
            graph.collect_items_in_space(&space()).len(),
            case.expect.item_count,
            "fixture case {} item count mismatch",
            case.id
        );
    }
}

fn apply_fixture_step(graph: &mut DockGraph, step: FixtureStep) {
    match step {
        FixtureStep::MoveItem {
            item,
            target_item,
            zone,
        } => {
            let item = DockItemId::new(item);
            let (source_tabs, _) = graph
                .find_item_in_space(&space(), &item)
                .expect("fixture source item should be findable");
            let target_tabs = graph
                .find_item_in_space(&space(), &DockItemId::new(target_item))
                .expect("fixture target item should be findable")
                .0;
            let zone: DropZone = zone.into();
            let target = if zone == DropZone::Center {
                DockGraphDropTarget::center(target_tabs)
            } else {
                DockGraphDropTarget::inner_edge(
                    graph.root(&space()).expect("fixture should have a root"),
                    target_tabs,
                    zone,
                )
            };
            let changed = graph
                .apply_op_checked(&DockOp::MoveItem {
                    source_space: space(),
                    item,
                    target_space: space(),
                    target,
                })
                .expect("fixture move_item should commit transactionally");
            assert_eq!(
                changed,
                !(source_tabs == target_tabs && zone == DropZone::Center),
                "fixture move_item changed-state mismatch"
            );
        }
        FixtureStep::FloatItem { item, bounds } => {
            assert!(
                graph
                    .apply_op_checked(&DockOp::FloatItemInWindow {
                        source_space: space(),
                        item: DockItemId::new(item),
                        target_space: space(),
                        bounds: dock_bounds(bounds[0], bounds[1], bounds[2], bounds[3]),
                    })
                    .expect("fixture float_item should commit")
            );
        }
        FixtureStep::MergeFirstFloating { target_item } => {
            let floating = graph
                .floating_containers(&space())
                .first()
                .expect("fixture should have a floating container")
                .node;
            let target_tabs = graph
                .find_item_in_space(&space(), &DockItemId::new(target_item))
                .expect("fixture merge target item should be findable")
                .0;
            assert!(
                graph
                    .apply_op_checked(&DockOp::MoveFloating {
                        source_space: space(),
                        floating,
                        target_space: space(),
                        target: DockGraphDropTarget::center(target_tabs),
                    })
                    .expect("fixture floating merge should commit")
            );
        }
    }
}

fn assert_unique_items(graph: &DockGraph, case_id: &str) {
    let items = graph.collect_items_in_space(&space());
    let mut unique = HashSet::new();
    for item in &items {
        assert!(
            unique.insert(item.clone()),
            "fixture case {case_id} duplicated item {item}"
        );
    }
}
