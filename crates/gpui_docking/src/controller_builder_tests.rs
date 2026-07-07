use crate::graph_test_support::{item, main_space as space};
use crate::*;

#[test]
fn controller_builder_sets_layout_panels_policy_and_options() {
    let options = DockHostOptions {
        empty_message: "No panels".to_string(),
        ..DockHostOptions::default()
    };

    let controller = DockController::builder(space())
        .default_editor_layout(EditorDockLayoutSpec::new(
            ["explorer"],
            ["editor", "preview"],
            ["terminal"],
        ))
        .panel_factory("explorer", "Explorer", |_| {
            unreachable!("lazy panel factories should not run during controller setup")
        })
        .panel_factory("editor", "Editor", |_| {
            unreachable!("lazy panel factories should not run during controller setup")
        })
        .panel_descriptor(
            "terminal",
            DockPanelDescriptor::new("Terminal")
                .closable(false)
                .with_dock_class("tool"),
        )
        .allow_dock_class_in_space(space(), "tool")
        .allow_floating(true)
        .allow_platform_viewports(true)
        .platform_focus_sets_dock_focus(false)
        .options(options)
        .build();

    assert_eq!(controller.space(), &space());
    assert!(controller.graph().root(&space()).is_some());
    assert!(controller.policy().allows_floating());
    assert!(controller.policy().allows_platform_viewports());
    assert!(!controller.policy().platform_focus_sets_dock_focus());
    assert_eq!(controller.options().empty_message, "No panels");

    let explorer = controller
        .panels()
        .get(&item("explorer"))
        .expect("builder should register explorer panel");
    assert_eq!(explorer.title(), "Explorer");
    assert!(controller.panels().has_view_lifecycle(&item("explorer")));
    assert!(controller.panels().contains(&item("editor")));
    let terminal = controller
        .panels()
        .descriptor(&item("terminal"))
        .expect("builder should register descriptor-only metadata");
    assert_eq!(terminal.title(), "Terminal");
    assert!(!terminal.is_closable());
    assert_eq!(terminal.dock_class(), Some(&DockClassId::from("tool")));
    assert!(
        !controller.panels().has_view_lifecycle(&item("terminal")),
        "descriptor-only builder entries should not bind view lifecycle"
    );
    assert!(
        controller
            .policy()
            .allows_dock_class_in_space(&space(), Some(&DockClassId::from("tool")))
    );
}

#[test]
fn controller_builder_creates_layout_from_product_panel_placements() {
    let controller = DockController::builder(space())
        .panel_placements([
            DockPanelPlacement::center("editor").selected(),
            DockPanelPlacement::stacked_with("preview", "editor"),
            DockPanelPlacement::left_rail("explorer").fraction(0.24),
            DockPanelPlacement::right_rail("inspector").fraction(0.22),
            DockPanelPlacement::bottom_rail("terminal").fraction(0.30),
        ])
        .build();
    let graph = controller.graph();
    let editor = item("editor");
    let preview = item("preview");
    let explorer = item("explorer");
    let inspector = item("inspector");
    let terminal = item("terminal");

    let (editor_tabs, editor_index) = graph
        .find_item_in_space(&space(), &editor)
        .expect("editor should be in the center stack");
    let (preview_tabs, preview_index) = graph
        .find_item_in_space(&space(), &preview)
        .expect("preview should stack with editor");
    assert_eq!(editor_tabs, preview_tabs);
    assert_eq!((editor_index, preview_index), (0, 1));
    assert_eq!(
        graph.selected_item_in_tabs(editor_tabs),
        Some(editor.clone())
    );
    assert_eq!(
        graph
            .central_region(&space())
            .and_then(|central| central.node),
        Some(editor_tabs),
        "center placement should mark the product center stack as the central region"
    );

    let (left_tabs, _) = graph
        .find_item_in_space(&space(), &explorer)
        .expect("left rail should be present");
    let (right_tabs, _) = graph
        .find_item_in_space(&space(), &inspector)
        .expect("right rail should be present");
    let (bottom_tabs, _) = graph
        .find_item_in_space(&space(), &terminal)
        .expect("bottom rail should be present");
    assert_ne!(left_tabs, editor_tabs);
    assert_ne!(right_tabs, editor_tabs);
    assert_ne!(bottom_tabs, editor_tabs);

    let root = graph
        .root(&space())
        .expect("placement layout should set a root");
    let DockNode::Split {
        axis: root_axis,
        children: root_children,
        fractions: root_fractions,
    } = graph.node(root).expect("root should exist")
    else {
        panic!("bottom rail should wrap the horizontal work area in a vertical split");
    };
    assert_eq!(*root_axis, SplitAxis::Vertical);
    assert_eq!(root_children.len(), 2);
    assert!((root_fractions[1] - 0.30).abs() <= 0.0001);

    let DockNode::Split {
        axis: work_area_axis,
        children: work_area_children,
        fractions: work_area_fractions,
    } = graph
        .node(root_children[0])
        .expect("work area should exist")
    else {
        panic!("left/right rails should compile into a horizontal work area split");
    };
    assert_eq!(*work_area_axis, SplitAxis::Horizontal);
    assert_eq!(
        work_area_children,
        &vec![left_tabs, editor_tabs, right_tabs]
    );
    assert!((work_area_fractions[0] - 0.24).abs() <= 0.0001);
    assert!((work_area_fractions[2] - 0.22).abs() <= 0.0001);
    assert_eq!(root_children[1], bottom_tabs);
}

#[test]
fn controller_builder_applies_panel_placement_fallback_fraction() {
    let controller = DockController::builder(space())
        .panel_placements([
            DockPanelPlacement::center("editor"),
            DockPanelPlacement::stacked_with("inspector", "missing-anchor")
                .fallback(DockPanelPlacementTarget::right_rail().fraction(0.31)),
        ])
        .build();
    let graph = controller.graph();
    let (editor_tabs, _) = graph
        .find_item_in_space(&space(), &item("editor"))
        .expect("center panel should be present");
    let (inspector_tabs, _) = graph
        .find_item_in_space(&space(), &item("inspector"))
        .expect("fallback rail panel should be present");

    let root = graph
        .root(&space())
        .expect("placement layout should set a root");
    let DockNode::Split {
        axis,
        children,
        fractions,
    } = graph.node(root).expect("root should exist")
    else {
        panic!("center plus right fallback should compile into a horizontal split");
    };
    assert_eq!(*axis, SplitAxis::Horizontal);
    assert_eq!(children, &vec![editor_tabs, inspector_tabs]);
    assert!((fractions[0] - 0.69).abs() <= 0.0001);
    assert!((fractions[1] - 0.31).abs() <= 0.0001);
}

#[test]
fn controller_opens_panel_by_product_placement_with_fallback() {
    let mut controller = DockController::builder(space())
        .panel_placements([
            DockPanelPlacement::center("editor"),
            DockPanelPlacement::right_rail("inspector"),
        ])
        .panel_descriptor("terminal", DockPanelDescriptor::new("Terminal"))
        .build();

    controller
        .open_item_at_placement(
            space(),
            DockPanelPlacement::stacked_with("terminal", "missing-anchor")
                .fallback(DockPanelPlacementTarget::right_rail()),
        )
        .expect("registered terminal should open through fallback placement");

    let graph = controller.graph();
    let (terminal_tabs, _) = graph
        .find_item_in_space(&space(), &item("terminal"))
        .expect("terminal should open");
    let (inspector_tabs, _) = graph
        .find_item_in_space(&space(), &item("inspector"))
        .expect("inspector should still be in the right rail");
    assert_eq!(
        terminal_tabs, inspector_tabs,
        "missing stack anchor should fall back to the requested right rail"
    );
}

#[test]
fn controller_builder_restores_valid_layout_and_rejects_invalid_layout() {
    let graph = DockGraph::default_editor_layout(
        space(),
        EditorDockLayoutSpec::new(["explorer"], ["editor"], ["terminal"]),
    );
    let layout = graph.export_layout();

    let controller = DockController::builder(space())
        .try_layout(&layout)
        .expect("valid dock layout should restore")
        .build();
    assert!(controller.graph().root(&space()).is_some());
    assert!(controller.panels().is_empty());

    let mut invalid = layout;
    invalid.layout_version = 99;
    assert_eq!(
        DockController::builder(space())
            .try_layout(&invalid)
            .expect_err("invalid layout version should be rejected"),
        DockLayoutValidationError::UnsupportedVersion {
            expected: DOCK_LAYOUT_VERSION,
            found: 99,
        }
    );
}

#[test]
fn controller_builder_restored_layout_keeps_panel_metadata_out_of_layout() {
    let graph = DockGraph::default_editor_layout(
        space(),
        EditorDockLayoutSpec::new(["explorer"], ["editor"], ["terminal"]),
    );
    let layout = graph.export_layout();

    let controller = DockController::builder(space())
        .try_layout(&layout)
        .expect("valid dock layout should restore")
        .panel_factory("editor", "Editor", |_| {
            unreachable!("lazy panel factories should not run during controller setup")
        })
        .allow_floating(true)
        .allow_platform_viewports(true)
        .build();

    let descriptor = controller
        .panels()
        .descriptor(&item("editor"))
        .expect("builder should register editor panel metadata");
    assert_eq!(descriptor.title(), "Editor");

    let exported = controller.graph().export_layout();
    exported
        .validate()
        .expect("restored builder layout should stay valid");
    let json = serde_json::to_string(&exported).expect("layout should serialize");
    assert!(
        json.contains("editor"),
        "layout should persist dock item ids"
    );
    assert!(
        !json.contains("Editor"),
        "layout should not persist panel metadata"
    );
    assert!(controller.policy().allows_floating());
    assert!(controller.policy().allows_platform_viewports());
}
