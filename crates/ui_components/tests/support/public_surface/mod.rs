use open_gpui_ui_components::component_contract::{
    COMPONENT_API_INVENTORY, COMPONENT_CONTRACT_ROWS, ComponentApiInventoryEntry,
    PUBLIC_SURFACE_OWNER_MAP, PublicSurfaceOwnerClass, SurfaceDocsStatus, SurfaceGalleryStatus,
    component_contract_entry, component_public_methods, component_recipe_component_rows,
    component_source_inputs, default_surface_rows, gallery_surface_rows, official_component_rows,
    official_overlay_component_rows, public_owner_for_component_inventory,
};
use open_gpui_ui_components::{ColorIntent, FocusRing, gpui_adapter::gpui_role_from_ui};
use open_gpui_ui_core::{
    AccessibleAction, Orientation, OverlayLayerKind, OverlayLayerPolicy, OverlayPlacementAlignment,
    OverlayPlacementSide, OverlayPresence, Role, UiPoint, UiPx, UiSize, rect, semantic, ui_point,
    ui_px, ui_size,
};
use open_gpui::{div, px};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SurfacePrimitiveStatus {
    NotPrimitive,
    PublicPrimitiveModule,
    RemovedPrimitiveModule,
}



#[derive(Debug, Clone, PartialEq, Eq)]
struct SurfaceManifestEntry {
    name: String,
    owner: PublicSurfaceOwnerClass,
    home: String,
    root_export: bool,
    prelude_export: bool,
    primitive_status: SurfacePrimitiveStatus,
    adapter_only: bool,
    diagnostic_only: bool,
    gallery_status: SurfaceGalleryStatus,
    docs_status: SurfaceDocsStatus,
    docs_token: Option<&'static str>,
}

fn contract_default_surface_tokens() -> std::collections::BTreeSet<String> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| entry.default_export)
        .map(|entry| entry.name.to_owned())
        .collect()
}

fn contract_non_default_surface_tokens() -> std::collections::BTreeSet<String> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .filter(|entry| !entry.default_export)
        .filter(|entry| !entry.name.contains("::"))
        .map(|entry| entry.name.to_owned())
        .collect()
}

fn component_public_methods_from_source(component: &str) -> Vec<String> {
    const MARKER_PREFIX: &str = "impl ";

    let marker = format!("{MARKER_PREFIX}{component} {{");
    let source_paths = component_source_paths(component);
    let mut methods = Vec::new();
    let mut found_impl = false;

    for source_path in &source_paths {
        let source = read_source_file(source_path);
        let mut search_start = 0usize;

        while let Some(relative_impl_start) = source[search_start..].find(&marker) {
            found_impl = true;
            let impl_start = search_start + relative_impl_start;
            let body_start = source[impl_start..]
                .find('{')
                .map(|offset| impl_start + offset)
                .expect("impl body should open with `{`");

            let mut depth = 0usize;
            let mut body_end = None;
            for (index, ch) in source[body_start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            body_end = Some(body_start + index);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let body_end = body_end.expect("impl body should close");
            let body = &source[body_start + 1..body_end];

            for line in body.lines() {
                let trimmed = line.trim_start();
                if let Some(signature) = trimmed.strip_prefix("pub const fn ") {
                    let before_paren = signature
                        .split_once('(')
                        .map(|(name, _)| name)
                        .unwrap_or(signature);
                    let name = before_paren
                        .split_once('<')
                        .map(|(name, _)| name)
                        .unwrap_or(before_paren)
                        .trim();
                    methods.push(name.to_string());
                } else if let Some(signature) = trimmed.strip_prefix("pub fn ") {
                    let before_paren = signature
                        .split_once('(')
                        .map(|(name, _)| name)
                        .unwrap_or(signature);
                    let name = before_paren
                        .split_once('<')
                        .map(|(name, _)| name)
                        .unwrap_or(before_paren)
                        .trim();
                    methods.push(name.to_string());
                }
            }

            search_start = body_end + 1;
        }
    }

    if !found_impl {
        panic!(
            "missing `{marker}` in component source mapping for `{component}`: {source_paths:?}"
        );
    }

    methods
}

fn component_source_paths(component: &str) -> Vec<std::path::PathBuf> {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();

    for source_entry in component_source_inputs(component) {
        let mapped_path = source_dir.join(source_entry);
        if mapped_path.is_file() {
            paths.push(mapped_path);
        } else if mapped_path.is_dir() {
            collect_rs_files(&mapped_path, &mut paths);
        } else if let Some(module_name) = source_entry.strip_suffix(".rs") {
            let mod_path = source_dir.join(module_name).join("mod.rs");
            if mod_path.is_file() {
                paths.push(mod_path);
            } else {
                panic!("component source input `{source_entry}` does not exist");
            }
        } else {
            panic!("component source input `{source_entry}` must be a .rs file or directory");
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn read_source_file(source_path: &std::path::Path) -> String {
    std::fs::read_to_string(source_path)
        .unwrap_or_else(|error| panic!("failed to read {source_path:?}: {error}"))
}

fn collect_rs_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read source dir {dir:?}: {error}"));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read source dir entry: {error}"))
            .path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn ui_component_source_files() -> Vec<std::path::PathBuf> {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&source_dir, &mut files);
    files.sort();
    files
}

fn surface_manifest() -> Vec<SurfaceManifestEntry> {
    let root_exports = default_reexport_tokens("lib.rs");
    let prelude_exports = default_reexport_tokens("prelude.rs");
    let mut entries = COMPONENT_CONTRACT_ROWS
        .iter()
        .map(|entry| SurfaceManifestEntry {
            name: entry.name.to_owned(),
            owner: entry.owner,
            home: entry.source_home.to_owned(),
            root_export: root_exports.contains(entry.name),
            prelude_export: prelude_exports.contains(entry.name),
            primitive_status: primitive_status_for_surface(entry.name, entry.source_home),
            adapter_only: entry.owner == PublicSurfaceOwnerClass::GpuiAdapterHelper,
            diagnostic_only: entry.owner == PublicSurfaceOwnerClass::DiagnosticSurface,
            gallery_status: component_gallery_status(entry.name)
                .unwrap_or(SurfaceGalleryStatus::NotInGallery),
            docs_status: entry.docs_status,
            docs_token: entry.docs_token,
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

fn component_gallery_status(name: &str) -> Option<SurfaceGalleryStatus> {
    component_contract_entry(name)
        .map(|entry| entry.gallery_status)
        .filter(|status| *status != SurfaceGalleryStatus::NotInGallery)
}

fn primitive_status_for_surface(name: &str, home: &str) -> SurfacePrimitiveStatus {
    if name.starts_with("primitives::") && home == "removed" {
        SurfacePrimitiveStatus::RemovedPrimitiveModule
    } else if name.starts_with("primitives::") {
        SurfacePrimitiveStatus::PublicPrimitiveModule
    } else {
        SurfacePrimitiveStatus::NotPrimitive
    }
}

fn component_api_entry(component: &str) -> &'static ComponentApiInventoryEntry {
    COMPONENT_API_INVENTORY
        .iter()
        .find(|entry| entry.component == component)
        .unwrap_or_else(|| panic!("missing component API inventory row for `{component}`"))
}

fn assert_inventory_contains_controlled_input(component: &str, input: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry.controlled_inputs.contains(&input),
        "{component} inventory should classify `{input}` as a controlled input"
    );
}

fn assert_inventory_contains_default_seed(component: &str, builder: &str, runtime_value: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry
            .default_seeds
            .iter()
            .any(|seed| seed.builder == builder && seed.runtime_value == runtime_value),
        "{component} inventory should classify `{builder}` as a default seed for `{runtime_value}`"
    );
}

fn assert_inventory_contains_callback(component: &str, name: &str, payload: &str) {
    let entry = component_api_entry(component);
    assert!(
        entry
            .callbacks
            .iter()
            .any(|callback| callback.name == name && callback.payload == payload),
        "{component} inventory should document callback `{name}` payload `{payload}`"
    );
}

fn public_primitive_modules_from_mod() -> Vec<String> {
    let source_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/primitives/mod.rs");
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {source_path:?}: {error}"));
    let mut modules = source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub mod ")
                .and_then(|module| module.strip_suffix(';'))
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    modules.sort();
    modules
}

fn default_reexport_tokens(file_name: &str) -> std::collections::BTreeSet<String> {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_path = manifest_dir.join("src").join(file_name);
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {file_name}: {error}"));
    let source = if file_name == "lib.rs" {
        source_without_gpui_adapter_module(&source)
    } else {
        source
    };
    reexport_tokens_from_source(&source, source_path.parent().expect("src directory should exist"))
}

fn reexport_tokens_from_source(
    source: &str,
    base_dir: &std::path::Path,
) -> std::collections::BTreeSet<String> {
    let mut exports = std::collections::BTreeSet::new();
    let mut statement = String::new();
    let mut collecting = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if collecting {
            statement.push(' ');
            statement.push_str(trimmed);
        } else if trimmed.starts_with("pub use ") {
            statement.clear();
            statement.push_str(trimmed);
            collecting = true;
        }

        if collecting && trimmed.ends_with(';') {
            collect_public_reexport_tokens(&statement, base_dir, &mut exports);
            statement.clear();
            collecting = false;
        }
    }

    exports
}

fn collect_public_reexport_tokens(
    statement: &str,
    base_dir: &std::path::Path,
    exports: &mut std::collections::BTreeSet<String>,
) {
    let statement = statement.trim().trim_end_matches(';');
    let Some(rest) = statement.strip_prefix("pub use ") else {
        return;
    };
    if rest.contains("::*") {
        collect_curated_wildcard_reexport_tokens(rest, base_dir, exports);
        return;
    }

    if let Some((_, group)) = rest.split_once("::{") {
        let group = group.trim_end_matches('}');
        for item in group.split(',') {
            collect_public_reexport_token(item, exports);
        }
    } else {
        collect_public_reexport_token(rest, exports);
    }
}

fn collect_curated_wildcard_reexport_tokens(
    rest: &str,
    base_dir: &std::path::Path,
    exports: &mut std::collections::BTreeSet<String>,
) {
    let Some(module_path) = rest.strip_suffix("::*") else {
        return;
    };
    let relative_module_path = module_path
        .strip_prefix("public_api::")
        .or_else(|| module_path.strip_prefix("crate::public_api::"));
    let Some(relative_module_path) = relative_module_path else {
        return;
    };
    let relative_module_path = relative_module_path.replace("::", "/");
    let source_path = base_dir
        .join("public_api")
        .join(format!("{relative_module_path}.rs"));
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read curated public API {source_path:?}: {error}"));
    exports.extend(reexport_tokens_from_source(&source, base_dir));
}

fn collect_public_reexport_token(item: &str, exports: &mut std::collections::BTreeSet<String>) {
    let token = item.trim();
    if token.is_empty() {
        return;
    }

    let exported_name = token
        .split_once(" as ")
        .map(|(_, alias)| alias.trim())
        .unwrap_or(token)
        .rsplit("::")
        .next()
        .unwrap_or(token)
        .trim();

    if !exported_name.is_empty() {
        exports.insert(exported_name.to_owned());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PublicContractBlocker {
    file: String,
    contract: String,
    token: String,
}

impl PublicContractBlocker {
    fn new(file: String, contract: String, token: String) -> Self {
        Self {
            file,
            contract,
            token,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PublicSurfaceBlocker {
    file: String,
    token: String,
}

impl PublicSurfaceBlocker {
    fn new(file: String, token: String) -> Self {
        Self { file, token }
    }
}

struct PublicContractStruct<'a> {
    name: &'a str,
    fields: &'a str,
}

fn public_contract_structs<'a>(
    source: &'a str,
    suffixes: &[&str],
) -> Vec<PublicContractStruct<'a>> {
    let mut states = Vec::new();
    let mut search_from = 0;

    while let Some(relative_start) = source[search_from..].find("pub struct ") {
        let start = search_from + relative_start;
        let name_start = start + "pub struct ".len();
        let name_end = source[name_start..]
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map(|offset| name_start + offset)
            .unwrap_or(source.len());
        let name = &source[name_start..name_end];

        search_from = name_end;
        if !suffixes.iter().any(|suffix| name.ends_with(suffix)) {
            continue;
        }
        if ["EmptyState"].contains(&name) {
            continue;
        }

        let Some(open_brace) = source[name_end..].find('{').map(|offset| name_end + offset) else {
            continue;
        };
        let Some(close_brace) = matching_brace(source, open_brace) else {
            continue;
        };

        states.push(PublicContractStruct {
            name,
            fields: &source[open_brace + 1..close_brace],
        });
        search_from = close_brace + 1;
    }

    states
}

fn public_contract_extraction_blockers(tokens: &[&str]) -> Vec<PublicContractBlocker> {
    let mut blockers = Vec::new();
    for source_file in ui_component_source_files() {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for contract in public_contract_structs(&source, &["State", "Metrics"]) {
            let fields = uncommented_lines(contract.fields);
            for token in tokens {
                if fields.contains(token) {
                    blockers.push(PublicContractBlocker::new(
                        file_name.to_owned(),
                        contract.name.to_owned(),
                        (*token).to_owned(),
                    ));
                }
            }
        }
    }

    blockers
}

fn public_surface_blockers(tokens: &[&str]) -> Vec<PublicSurfaceBlocker> {
    let mut blockers = Vec::new();
    for source_file in ui_component_source_files() {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        let source = if matches!(file_name, "lib.rs" | "prelude.rs") {
            source_without_gpui_adapter_module(&source)
        } else {
            source
        };
        let surface = public_api_surface(&uncommented_lines(&source));

        for token in tokens {
            if surface.contains(token) {
                blockers.push(PublicSurfaceBlocker::new(
                    file_name.to_owned(),
                    (*token).to_owned(),
                ));
            }
        }
    }

    blockers
}

fn source_without_gpui_adapter_module(source: &str) -> String {
    let Some((module_start, close_brace)) = public_module_bounds(source, "gpui_adapter") else {
        return source.to_owned();
    };

    let mut stripped = String::with_capacity(source.len());
    stripped.push_str(&source[..module_start]);
    stripped.push_str(&source[close_brace + 1..]);
    stripped
}

fn public_module_source<'a>(source: &'a str, module_name: &str) -> Option<&'a str> {
    let (module_start, close_brace) = public_module_bounds(source, module_name)?;
    Some(&source[module_start..=close_brace])
}

fn public_module_bounds(source: &str, module_name: &str) -> Option<(usize, usize)> {
    let module_marker = format!("pub mod {module_name}");
    let module_start = source.find(&module_marker)?;
    let open_brace = source[module_start..]
        .find('{')
        .map(|offset| module_start + offset)?;
    let close_brace = matching_brace(source, open_brace)?;
    Some((module_start, close_brace))
}

fn public_api_surface(source: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let mut surface = Vec::new();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        let line = lines[line_index];
        let trimmed = line.trim_start();

        if trimmed.starts_with("pub use ") {
            while line_index < lines.len() {
                let signature_line = lines[line_index];
                surface.push(signature_line);
                line_index += 1;
                if signature_line.contains(';') {
                    break;
                }
            }
            continue;
        }

        if trimmed.starts_with("pub fn ") {
            while line_index < lines.len() {
                let signature_line = lines[line_index];
                surface.push(signature_line);
                line_index += 1;
                if signature_line.contains('{') || signature_line.contains(';') {
                    break;
                }
            }
            continue;
        }

        if trimmed.starts_with("pub const ")
            || trimmed.starts_with("pub type ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("impl EntityInputHandler for ")
        {
            surface.push(line);
            line_index += 1;
            continue;
        }

        line_index += 1;
    }

    surface.join("\n")
}

fn matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (offset, ch) in source[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_brace + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn uncommented_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
