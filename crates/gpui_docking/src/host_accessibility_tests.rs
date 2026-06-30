use crate::{
    DropZone,
    accessibility_scene::{DockAccessibilityRole, DockAccessibilityScene},
    host_test_support::{floating_bounds, floating_overlay_graph, open_host},
    overlay_scene::{DockOverlayLayer, DockOverlayLayerKind, DockOverlayScene},
};
use open_gpui::{TestAppContext, px, size};
use open_gpui_ui_core::{AccessibleAction, Orientation};
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
