use crate::{
    DropZone, SplitAxis,
    accessibility_scene::{DockAccessibilityLayer, DockAccessibilityRole, DockAccessibilityScene},
    host_test_support::{floating_bounds, floating_overlay_graph, open_host},
    overlay_scene::{DockOverlayLayer, DockOverlayLayerKind, DockOverlayScene},
    transition_geometry::DockMotionPreference,
    zoom_state::DockZoomScene,
};
use open_gpui::{TestAppContext, px, size};
use open_gpui_ui_core::{AccessibleAction, Orientation, Role};
use slotmap::Key;

#[open_gpui::test]
fn accessibility_scene_enumerates_presentation_roles(cx: &mut TestAppContext) {
    let (graph, root, floating) = floating_overlay_graph();
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(300.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 500.0, 300.0), cx)
    });

    let accessibility = DockAccessibilityScene::from_presentation(&scene);
    let roles = accessibility
        .descriptors
        .iter()
        .map(|descriptor| descriptor.role)
        .collect::<Vec<_>>();

    assert!(roles.contains(&DockAccessibilityRole::Pane));
    assert!(roles.contains(&DockAccessibilityRole::TabList));
    assert!(roles.contains(&DockAccessibilityRole::Tab));
    assert!(roles.contains(&DockAccessibilityRole::TabPanel));
    assert!(roles.contains(&DockAccessibilityRole::FocusRegion));
    assert!(roles.contains(&DockAccessibilityRole::FloatingWindow));
    assert!(roles.contains(&DockAccessibilityRole::DragSource));
    assert!(roles.contains(&DockAccessibilityRole::DropDestination));
    assert!(
        accessibility
            .descriptors
            .iter()
            .any(
                |descriptor| descriptor.role == DockAccessibilityRole::FloatingWindow
                    && descriptor.node == Some(floating)
            )
    );
    assert!(
        accessibility
            .descriptors
            .iter()
            .any(|descriptor| descriptor.role == DockAccessibilityRole::Pane
                && descriptor.node == Some(root))
    );
    assert!(
        accessibility
            .descriptors
            .iter()
            .any(
                |descriptor| descriptor.role == DockAccessibilityRole::TabPanel
                    && descriptor.label.as_deref() == Some("Panel A panel")
            )
    );
    assert!(
        accessibility
            .descriptors
            .iter()
            .filter(|descriptor| descriptor.role == DockAccessibilityRole::TabPanel)
            .all(
                |descriptor| !descriptor.label.as_deref().is_some_and(|label| {
                    label.starts_with("Panel ") && !label.ends_with(" panel")
                })
            )
    );
}

#[open_gpui::test]
fn accessibility_scene_enumerates_splitters(cx: &mut TestAppContext) {
    let (graph, root, _left, _right) =
        crate::host_test_support::split_graph(crate::SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 400.0, 220.0), cx)
    });
    let accessibility = DockAccessibilityScene::from_presentation(&scene);

    assert!(
        accessibility
            .descriptors
            .iter()
            .any(
                |descriptor| descriptor.role == DockAccessibilityRole::Splitter
                    && descriptor.node == Some(root)
                    && descriptor.orientation == Some(Orientation::Horizontal)
                    && descriptor.disabled == Some(false)
                    && descriptor.actions
                        == vec![AccessibleAction::Increment, AccessibleAction::Decrement]
            )
    );
}

#[open_gpui::test]
fn accessibility_gpui_mapping_exposes_stable_roles_ids_and_final_tab_actions(
    cx: &mut TestAppContext,
) {
    let (graph, _root, _floating) = floating_overlay_graph();
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(300.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 500.0, 300.0), cx)
    });
    let accessibility = DockAccessibilityScene::from_presentation(&scene);
    let elements = accessibility.gpui_elements(DockAccessibilityLayer::Final);

    let tab = elements
        .iter()
        .find(|element| element.role == Role::Tab && element.label == "Panel A")
        .expect("panel tab should map to a GPUI tab element");
    assert_eq!(tab.gpui_role, open_gpui::Role::Tab);
    assert!(tab.id_str().starts_with("dock-a11y:final:tab:"));
    assert_eq!(tab.selected, Some(true));
    assert_eq!(tab.disabled, false);
    assert!(tab.has_action(AccessibleAction::Click));
    assert!(tab.has_action(AccessibleAction::Focus));
    assert_eq!(tab.hint.as_deref(), Some("Activate to select this tab"));

    let tab_list = elements
        .iter()
        .find(|element| element.role == Role::TabList && element.node == tab.node)
        .expect("tab list should map to GPUI tab-list element");
    assert_eq!(tab_list.gpui_role, open_gpui::Role::TabList);
    assert_eq!(tab_list.orientation, Some(Orientation::Horizontal));
    assert!(tab_list.actions.is_empty());

    let panel = elements
        .iter()
        .find(|element| element.role == Role::TabPanel && element.item == tab.item)
        .expect("selected panel should map to a GPUI tab panel element");
    assert_eq!(panel.gpui_role, open_gpui::Role::TabPanel);
    assert_eq!(panel.label, "Panel A panel");
    assert_eq!(panel.selected, Some(true));
    assert!(panel.has_action(AccessibleAction::Focus));

    let ids = elements
        .iter()
        .map(|element| element.id_str().to_string())
        .collect::<Vec<_>>();
    let mut sorted_ids = ids.clone();
    sorted_ids.sort();
    sorted_ids.dedup();
    assert_eq!(ids.len(), sorted_ids.len(), "GPUI a11y IDs must be unique");
    let repeated_ids = accessibility
        .gpui_elements(DockAccessibilityLayer::Final)
        .into_iter()
        .map(|element| element.id_str().to_string())
        .collect::<Vec<_>>();
    assert_eq!(ids, repeated_ids, "GPUI a11y order must be deterministic");
}

#[open_gpui::test]
fn accessibility_gpui_mapping_exposes_splitter_state_and_actions(cx: &mut TestAppContext) {
    let (graph, root, _left, _right) =
        crate::host_test_support::split_graph(crate::SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 400.0, 220.0), cx)
    });
    let accessibility = DockAccessibilityScene::from_presentation(&scene);
    let elements = accessibility.gpui_elements(DockAccessibilityLayer::Final);
    let splitter = elements
        .iter()
        .find(|element| element.role == Role::Splitter)
        .expect("splitter should map to a GPUI splitter element");

    assert_eq!(splitter.gpui_role, open_gpui::Role::Splitter);
    assert!(splitter.id_str().starts_with("dock-a11y:final:splitter:"));
    assert_eq!(splitter.node, Some(root));
    assert_eq!(splitter.orientation, Some(Orientation::Horizontal));
    assert_eq!(splitter.numeric_value, Some(0.0));
    assert_eq!(splitter.hint.as_deref(), Some("Resize adjacent dock panes"));
    assert!(splitter.has_action(AccessibleAction::Increment));
    assert!(splitter.has_action(AccessibleAction::Decrement));
    assert_eq!(splitter.disabled, false);
}

#[open_gpui::test]
fn accessibility_final_semantics_match_reduced_and_animated_zoom(cx: &mut TestAppContext) {
    let (graph, _root, left, _right) =
        crate::host_test_support::split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 400.0, 220.0), cx)
    });

    let animated = DockZoomScene::from_scene(&scene, left, DockMotionPreference::Animated)
        .expect("animated zoom should resolve");
    let reduced = DockZoomScene::from_scene(&scene, left, DockMotionPreference::Reduced)
        .expect("reduced zoom should resolve");

    assert!(!animated.immediate);
    assert!(reduced.immediate);
    assert_eq!(
        gpui_accessibility_signature(&animated.scene),
        gpui_accessibility_signature(&reduced.scene),
        "reduced motion should only change timing, not final accessibility semantics"
    );
}

#[open_gpui::test]
fn accessibility_scene_marks_selected_tab_from_focus_region(cx: &mut TestAppContext) {
    let (graph, _root, _floating) = floating_overlay_graph();
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(500.0), px(300.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 500.0, 300.0), cx)
    });
    let selected_tab_node = scene
        .tab_labels
        .iter()
        .find(|tab| tab.title == "Panel A")
        .expect("selected floating tab label should be present")
        .tabs;
    let accessibility = DockAccessibilityScene::from_presentation(&scene);

    assert!(
        accessibility
            .descriptors
            .iter()
            .any(|descriptor| descriptor.role == DockAccessibilityRole::Tab
                && descriptor.node == Some(selected_tab_node)
                && descriptor.label.as_deref() == Some("Panel A")
                && descriptor.selected == Some(true)
                && descriptor.actions.contains(&AccessibleAction::Click)
                && descriptor.actions.contains(&AccessibleAction::Focus))
    );
}

#[test]
fn accessibility_scene_adds_overlay_drop_and_rejected_descriptors() {
    let tabs = crate::DockNodeId::null();
    let scene = DockOverlayScene {
        layers: vec![
            DockOverlayLayer {
                kind: DockOverlayLayerKind::TabInsertion,
                bounds: floating_bounds(8.0, 0.0, 3.0, 26.0),
                target_node: Some(tabs),
                zone: Some(DropZone::Center),
                preview_layer: None,
                active: true,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            },
            DockOverlayLayer {
                kind: DockOverlayLayerKind::RejectedState,
                bounds: floating_bounds(0.0, 0.0, 320.0, 200.0),
                target_node: Some(tabs),
                zone: None,
                preview_layer: None,
                active: true,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            },
        ],
    };
    let accessibility = DockAccessibilityScene {
        descriptors: Vec::new(),
    }
    .with_overlay(&scene);

    assert!(
        accessibility
            .descriptors
            .iter()
            .any(
                |descriptor| descriptor.role == DockAccessibilityRole::DropTarget
                    && descriptor.node == Some(tabs)
                    && descriptor.zone == Some(DropZone::Center)
                    && descriptor.disabled == Some(false)
                    && descriptor.actions == vec![AccessibleAction::CustomAction]
            )
    );
    assert!(
        accessibility
            .descriptors
            .iter()
            .any(
                |descriptor| descriptor.role == DockAccessibilityRole::RejectedDropTarget
                    && descriptor.node == Some(tabs)
                    && descriptor.disabled == Some(true)
                    && descriptor.actions.is_empty()
            )
    );
}

#[test]
fn accessibility_gpui_mapping_keeps_overlay_drop_affordances_short_lived_and_non_committing() {
    let tabs = crate::DockNodeId::null();
    let overlay = DockOverlayScene {
        layers: vec![
            DockOverlayLayer {
                kind: DockOverlayLayerKind::GuideBox,
                bounds: floating_bounds(0.0, 0.0, 120.0, 80.0),
                target_node: Some(tabs),
                zone: Some(DropZone::Left),
                preview_layer: None,
                active: true,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            },
            DockOverlayLayer {
                kind: DockOverlayLayerKind::PayloadGhost,
                bounds: floating_bounds(2.0, 2.0, 90.0, 24.0),
                target_node: Some(tabs),
                zone: Some(DropZone::Center),
                preview_layer: None,
                active: true,
                payload_index: Some(0),
                payload_title: Some("Panel A".to_string()),
                drop_box: None,
                tab_insertion: None,
            },
            DockOverlayLayer {
                kind: DockOverlayLayerKind::RejectedState,
                bounds: floating_bounds(0.0, 0.0, 120.0, 80.0),
                target_node: Some(tabs),
                zone: Some(DropZone::Right),
                preview_layer: None,
                active: true,
                payload_index: None,
                payload_title: None,
                drop_box: None,
                tab_insertion: None,
            },
        ],
    };
    let elements = DockAccessibilityScene::overlay_elements_for_render(&overlay);

    let drop_destination = elements
        .iter()
        .find(|element| {
            element
                .id_str()
                .starts_with("dock-a11y:overlay:drop-destination:")
        })
        .expect("active overlay should expose a drop destination descriptor");
    assert_eq!(drop_destination.role, Role::Group);
    assert_eq!(drop_destination.gpui_role, open_gpui::Role::Group);
    assert_eq!(drop_destination.zone, Some(DropZone::Left));
    assert_eq!(
        drop_destination.hint.as_deref(),
        Some("Drop target for left side")
    );
    assert_eq!(drop_destination.disabled, false);
    assert!(
        drop_destination.actions.is_empty(),
        "GPUI-facing drop affordance should not invent a platform drop action"
    );

    let drag_source = elements
        .iter()
        .find(|element| {
            element
                .id_str()
                .starts_with("dock-a11y:overlay:drag-source:")
        })
        .expect("payload ghost should expose an active drag source descriptor");
    assert_eq!(drag_source.label, "Dragging Panel A");
    assert_eq!(drag_source.hint.as_deref(), Some("Drag this dock item"));
    assert!(drag_source.actions.is_empty());

    let rejected = elements
        .iter()
        .find(|element| {
            element
                .id_str()
                .starts_with("dock-a11y:overlay:rejected-drop-target:")
        })
        .expect("rejected overlay should expose disabled descriptor");
    assert_eq!(rejected.disabled, true);
    assert_eq!(
        rejected.hint.as_deref(),
        Some("This dock target cannot accept the current payload")
    );
    assert!(rejected.actions.is_empty());

    let cleaned = DockAccessibilityScene::overlay_elements_for_render(&DockOverlayScene {
        layers: Vec::new(),
    });
    assert!(cleaned.is_empty());
}

#[open_gpui::test]
fn accessibility_splitter_actions_resize_through_transaction_path(cx: &mut TestAppContext) {
    let (graph, root, _left, _right) =
        crate::host_test_support::split_graph(SplitAxis::Horizontal, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(400.0), px(220.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 400.0, 220.0), cx)
    });
    host.update(cx, |host, _| host.set_last_presentation_scene(scene));

    let resized = host.update(cx, |host, cx| {
        host.resize_splitter_from_accessibility(
            root,
            SplitAxis::Horizontal,
            0,
            AccessibleAction::Increment,
            cx,
        )
    });
    assert!(resized, "increment should commit a resize transaction");

    host.update(cx, |host, cx| {
        host.with_workspace(cx, |workspace| {
            let Some(crate::DockNode::Split { fractions, .. }) = workspace.graph().node(root)
            else {
                panic!("root should remain a split");
            };
            assert!(
                fractions[0] > 0.5,
                "increment should grow the first adjacent pane through workspace resize"
            );
        });
    });
}

#[open_gpui::test]
fn accessibility_vertical_splitter_actions_target_vertical_axis(cx: &mut TestAppContext) {
    let (graph, root, _top, _bottom) =
        crate::host_test_support::split_graph(SplitAxis::Vertical, 0.5, 0.5);
    let (_window, host, _visual) = open_host(
        cx,
        graph,
        &[("a", "Panel A", "A"), ("b", "Panel B", "B")],
        size(px(320.0), px(400.0)),
    );
    let scene = host.update(cx, |host, cx| {
        host.presentation_scene_for_test(floating_bounds(0.0, 0.0, 320.0, 400.0), cx)
    });
    host.update(cx, |host, _| host.set_last_presentation_scene(scene));

    let resized = host.update(cx, |host, cx| {
        host.resize_splitter_from_accessibility(
            root,
            SplitAxis::Vertical,
            0,
            AccessibleAction::Decrement,
            cx,
        )
    });
    assert!(
        resized,
        "decrement should commit a vertical resize transaction"
    );

    host.update(cx, |host, cx| {
        host.with_workspace(cx, |workspace| {
            let Some(crate::DockNode::Split { fractions, .. }) = workspace.graph().node(root)
            else {
                panic!("root should remain a vertical split");
            };
            assert!(
                fractions[0] < 0.5,
                "vertical decrement should shrink the first adjacent pane"
            );
        });
    });
}

fn gpui_accessibility_signature(
    scene: &crate::presentation_scene::DockPresentationScene,
) -> Vec<(
    Role,
    String,
    Option<u64>,
    Option<String>,
    Option<Orientation>,
    Option<bool>,
    bool,
    Vec<AccessibleAction>,
)> {
    DockAccessibilityScene::from_presentation(scene)
        .gpui_elements(DockAccessibilityLayer::Final)
        .into_iter()
        .map(|element| {
            (
                element.role,
                element.label,
                element.node.map(|node| node.as_u64()),
                element.item.map(|item| item.to_string()),
                element.orientation,
                element.selected,
                element.disabled,
                element.actions,
            )
        })
        .collect()
}
