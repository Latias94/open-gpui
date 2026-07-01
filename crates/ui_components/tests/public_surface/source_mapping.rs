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
