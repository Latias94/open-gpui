fn read_source_file(file_name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(file_name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"))
}

fn source_line_contains_identifier(line: &str, token: &str) -> bool {
    line.split(|character: char| character != '_' && !character.is_ascii_alphanumeric())
        .any(|part| part == token)
}

fn source_contains_public_use_token(file_name: &str, token: &str) -> bool {
    let source = read_source_file(file_name);
    let mut in_public_use = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub use ") {
            in_public_use = true;
        }
        if in_public_use && source_line_contains_identifier(trimmed, token) {
            return true;
        }
        if in_public_use && trimmed.ends_with(';') {
            in_public_use = false;
        }
    }
    false
}

#[test]
fn root_facade_import_paths_compile() {
    use crate as root;

    let _ = (
        std::mem::size_of::<root::CanvasDocument>(),
        std::mem::size_of::<root::CanvasDocumentBuilder>(),
        std::mem::size_of::<root::CanvasSnapshot>(),
        std::mem::size_of::<root::CanvasRuntime>(),
        std::mem::size_of::<root::CanvasPaintFrame>(),
        std::mem::size_of::<root::CanvasEditor>(),
        std::mem::size_of::<root::CanvasStore>(),
        std::mem::size_of::<root::CanvasKindRegistry>(),
        std::mem::size_of::<root::CanvasViewport>(),
        std::mem::size_of::<root::JsonCanvas>(),
    );
}

#[test]
fn root_exports_core_facade_tokens_explicitly() {
    for token in [
        "CanvasDocument",
        "CanvasDocumentBuilder",
        "CanvasSnapshot",
        "CanvasRuntime",
        "CanvasPaintFrame",
        "CanvasEditor",
        "CanvasStore",
        "CanvasKindRegistry",
        "CanvasViewport",
        "JsonCanvas",
    ] {
        assert!(
            source_contains_public_use_token("lib.rs", token),
            "canvas root should explicitly export {token}"
        );
    }
}

#[test]
fn root_does_not_make_split_modules_public() {
    let source = read_source_file("lib.rs");
    let forbidden_public_modules = [
        "changes",
        "clipboard",
        "document",
        "format",
        "geometry",
        "geometry_facts",
        "gesture",
        "gpui",
        "graph",
        "json_canvas",
        "layer",
        "mutation",
        "persistence",
        "record_scope",
        "relations",
        "routing",
        "runtime",
        "runtime_query",
        "schema",
        "session",
        "snap",
        "spatial_cache",
        "store",
        "tool",
        "transform",
    ];

    for module in forbidden_public_modules {
        let marker = format!("pub mod {module};");
        assert!(
            !source.lines().any(|line| line.trim() == marker),
            "canvas internal module `{module}` must not become a public module"
        );
    }

    assert!(
        source.lines().any(|line| line.trim() == "pub mod index;"),
        "index remains the only doc-hidden public module on the canvas root"
    );
}

#[test]
fn root_reexports_stay_explicit_without_wildcards() {
    let source = read_source_file("lib.rs");
    let wildcard_lines = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            (line.contains("pub use ") && line.contains("::*"))
                .then_some(format!("lib.rs:{}", index + 1))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        wildcard_lines,
        Vec::<String>::new(),
        "canvas root public re-exports must stay explicit"
    );
}
