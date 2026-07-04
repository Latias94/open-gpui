use super::*;

#[test]
fn table_component_source_mapping_tracks_split_render_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    assert!(
        !source_dir.join("table.rs").exists(),
        "Table should resolve through table/mod.rs instead of the old single-file adapter"
    );
    assert_eq!(
        component_source_inputs("Table"),
        ["table/mod.rs", "table/resolve.rs"]
    );

    for owner in table_render_owner_files() {
        assert!(
            source_dir.join(owner).is_file(),
            "split Table render owner `{owner}` should exist"
        );
    }
}

#[test]
fn command_component_source_mapping_tracks_split_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let command_sources = [
        "command/mod.rs",
        "command/descriptor.rs",
        "command/model.rs",
        "command/render_plan.rs",
        "command/runtime.rs",
        "command/style.rs",
    ];

    assert!(!source_dir.join("command.rs").exists());
    assert_eq!(component_source_inputs("Command"), command_sources);

    for owner in command_sources {
        assert!(
            source_dir.join(owner).is_file(),
            "split Command owner `{owner}` should exist"
        );
    }
}

#[test]
fn menu_component_source_mapping_tracks_split_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let menu_sources = [
        "menu/mod.rs",
        "menu/descriptor.rs",
        "menu/model.rs",
        "menu/render_plan.rs",
        "menu/runtime.rs",
        "menu/style.rs",
    ];

    assert!(!source_dir.join("menu.rs").exists());
    assert_eq!(component_source_inputs("Menu"), menu_sources);

    for owner in menu_sources {
        assert!(
            source_dir.join(owner).is_file(),
            "split Menu owner `{owner}` should exist"
        );
    }
}

#[test]
fn context_menu_component_source_mapping_tracks_split_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let context_menu_sources = [
        "context_menu/mod.rs",
        "context_menu/model.rs",
        "menu/mod.rs",
        "menu/descriptor.rs",
        "menu/model.rs",
        "menu/runtime.rs",
        "menu/style.rs",
    ];

    assert!(!source_dir.join("context_menu.rs").exists());
    assert_eq!(component_source_inputs("ContextMenu"), context_menu_sources);

    for owner in context_menu_sources {
        assert!(
            source_dir.join(owner).is_file(),
            "split ContextMenu owner `{owner}` should exist"
        );
    }
}

#[test]
fn tree_component_source_mapping_tracks_split_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let tree_sources = [
        "tree/mod.rs",
        "tree/descriptor.rs",
        "tree/model.rs",
        "tree/runtime.rs",
        "tree/style.rs",
        "tree/movement.rs",
        "tree/render_plan.rs",
    ];
    let tree_state_sources = [
        "tree/model.rs",
        "tree/descriptor.rs",
        "tree/style.rs",
        "tree/movement.rs",
    ];

    assert!(!source_dir.join("tree.rs").exists());
    assert_eq!(component_source_inputs("Tree"), tree_sources);
    assert_eq!(component_source_inputs("TreeState"), tree_state_sources);

    for owner in tree_sources {
        assert!(
            source_dir.join(owner).is_file(),
            "split Tree owner `{owner}` should exist"
        );
    }
}

#[test]
fn select_component_source_mapping_tracks_split_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let select_sources = [
        "select/mod.rs",
        "select/model.rs",
        "select/render_plan.rs",
        "select/runtime.rs",
        "select/style.rs",
    ];

    assert!(!source_dir.join("select.rs").exists());
    assert_eq!(component_source_inputs("Select"), select_sources);

    for owner in select_sources {
        assert!(
            source_dir.join(owner).is_file(),
            "split Select owner `{owner}` should exist"
        );
    }
}

#[test]
fn table_behavior_source_mapping_tracks_split_owners() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let behavior_sources = [
        "table/behavior/mod.rs",
        "table/behavior/counts.rs",
        "table/behavior/columns.rs",
        "table/behavior/header.rs",
        "table/behavior/rows.rs",
        "table/behavior/tree.rs",
    ];

    assert!(
        !source_dir.join("table").join("behavior.rs").exists(),
        "TableBehaviorSnapshot should resolve through the split table behavior module instead of the old single-file owner"
    );
    assert_eq!(
        component_source_inputs("TableBehaviorSnapshot"),
        behavior_sources
    );

    for owner in behavior_sources {
        assert!(
            source_dir.join(owner).is_file(),
            "split Table behavior owner `{owner}` should exist"
        );
    }
}

#[test]
fn component_source_mapping_expands_split_component_directories() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source_files = component_source_paths("TableRangeFilter")
        .into_iter()
        .map(|path| {
            path.strip_prefix(&source_dir)
                .unwrap_or_else(|error| panic!("failed to strip source dir from {path:?}: {error}"))
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();

    assert!(
        source_files.contains(&"table/range_filter/component.rs".to_string()),
        "split component directory mapping should include its public component file"
    );
    assert!(
        source_files.contains(&"table/range_filter/state.rs".to_string()),
        "split component directory mapping should include adjacent public contract files"
    );
}
