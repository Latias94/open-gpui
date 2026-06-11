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
        .options(options)
        .build();

    assert_eq!(controller.space(), &space());
    assert!(controller.graph().root(&space()).is_some());
    assert!(controller.policy().allows_floating());
    assert!(controller.policy().allows_platform_viewports());
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
