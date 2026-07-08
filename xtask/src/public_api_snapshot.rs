use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(crate) fn scan_public_api(root: &Path, args: &[String]) -> Result<(), ()> {
    for arg in args {
        if arg != "--check" {
            eprintln!("unknown scan-public-api argument: {arg}");
            return Err(());
        }
    }

    println!("==> scan public API tiers");

    let failures = public_api_failures(root);
    if failures.is_empty() {
        println!("public API tier scan passed");
        Ok(())
    } else {
        eprintln!("public API tier scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn public_api_failures(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    scan_docking_public_api(root, &mut failures);
    scan_motion_public_api(root, &mut failures);
    scan_ui_components_public_api(root, &mut failures);
    scan_ui_core_public_api(root, &mut failures);
    failures
}

fn scan_docking_public_api(root: &Path, failures: &mut Vec<String>) {
    let source_dir = root.join("crates/gpui_docking/src");
    let forbidden_common = [
        "DockAction",
        "DockActionApplyError",
        "DockActionOutcome",
        "DockCentralRegion",
        "DockController",
        "DockControllerBuilder",
        "DockEdgeDockPlan",
        "DockFloatingContainer",
        "DockGraph",
        "DockGraphMutationError",
        "DockGraphValidationError",
        "DockHost",
        "DockHostOptions",
        "DockLayoutBuilder",
        "DockLayoutCentralRegion",
        "DockLayoutFloatingContainer",
        "DockLayoutNode",
        "DockLayoutSpace",
        "DockNode",
        "DockNodeId",
        "DockSpatialDirection",
        "DockSplitResize",
        "DockViewportRuntimeHandle",
        "DockWorkspace",
        "DropZone",
        "EditorDockLayoutSpec",
        "SplitAxis",
    ];

    for file_name in ["lib.rs", "prelude.rs"] {
        let exports = default_reexport_tokens(&source_dir, file_name, failures);
        reject_tokens(
            "crates/gpui_docking",
            file_name,
            &exports,
            &forbidden_common,
            "common surface must not export low-level docking model/runtime types",
            failures,
        );
    }
    reject_public_module(
        &source_dir.join("lib.rs"),
        "layout",
        "raw layout anatomy must stay behind the model tier",
        failures,
    );

    let facade_forbidden = [
        "DockAction",
        "DockController",
        "DockControllerBuilder",
        "DockHost",
        "DockHostOptions",
        "DockNodeId",
        "DockViewportRuntimeHandle",
        "DockWorkspace",
    ];
    for file_name in [
        "surface.rs",
        "surface/builder.rs",
        "surface/panel.rs",
        "surface/viewport.rs",
    ] {
        let tokens = public_signature_tokens(&source_dir.join(file_name), failures);
        reject_tokens(
            "crates/gpui_docking",
            file_name,
            &tokens,
            &facade_forbidden,
            "DockSurface facade signatures must stay semantic",
            failures,
        );
    }
}

fn scan_motion_public_api(root: &Path, failures: &mut Vec<String>) {
    let source_dir = root.join("crates/motion/src");
    let root_exports = default_reexport_tokens(&source_dir, "lib.rs", failures);
    let forbidden_root = [
        "MotionExecutionPlan",
        "MotionExecutionState",
        "MotionFrameHost",
        "MotionFrameHostSample",
        "MotionFrameHostUpdate",
        "MotionModel",
        "MotionPreset",
        "MotionProgressExecution",
        "MotionScalarController",
        "MotionScalarExecution",
        "MotionScalarTrack",
        "MotionSequence",
        "MotionSpec",
        "MotionSpring",
        "MotionSpringPreset",
        "MotionSpringSpec",
        "MotionTimeline",
    ];

    reject_tokens(
        "crates/motion",
        "lib.rs",
        &root_exports,
        &forbidden_root,
        "motion root must favor facade types over low-level execution/model internals",
        failures,
    );
}

fn scan_ui_components_public_api(root: &Path, failures: &mut Vec<String>) {
    let source_dir = root.join("crates/ui_components/src");
    let root_exports = default_reexport_tokens(&source_dir, "lib.rs", failures);
    let prelude_exports = default_reexport_tokens(&source_dir, "prelude.rs", failures);
    let default_exports = default_reexport_tokens(&source_dir, "public_api/default.rs", failures);
    let common_exports = default_reexport_tokens(&source_dir, "public_api/common.rs", failures);
    let contract_rows = component_contract_rows(&source_dir, failures);
    let contract_names = contract_rows
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let default_contract_names = contract_rows
        .iter()
        .filter_map(|(name, row)| row.default_export.then_some(name.as_str()))
        .collect::<BTreeSet<_>>();

    let leaked_non_default = root_exports
        .iter()
        .filter(|token| contract_names.contains(token.as_str()))
        .filter(|token| !default_contract_names.contains(token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !leaked_non_default.is_empty() {
        failures.push(format!(
            "crates/ui_components/src/lib.rs: root exports leaked non-default contract rows: {}",
            leaked_non_default.join(", ")
        ));
    }

    let missing_root_defaults = default_contract_names
        .iter()
        .filter(|token| !root_exports.contains(**token))
        .copied()
        .collect::<Vec<_>>();
    if !missing_root_defaults.is_empty() {
        failures.push(format!(
            "crates/ui_components/src/lib.rs: root exports are missing default contract rows: {}",
            missing_root_defaults.join(", ")
        ));
    }

    let prelude_only = prelude_exports
        .difference(&root_exports)
        .filter(|token| !ui_components_prelude_helper_allowlist().contains(token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !prelude_only.is_empty() {
        failures.push(format!(
            "crates/ui_components/src/prelude.rs: prelude-only exports need an explicit helper allowlist entry: {}",
            prelude_only.join(", ")
        ));
    }

    let common_extra = common_exports
        .difference(&default_exports)
        .cloned()
        .collect::<Vec<_>>();
    if !common_extra.is_empty() {
        failures.push(format!(
            "crates/ui_components/src/public_api/common.rs: common exports must stay a subset of default exports: {}",
            common_extra.join(", ")
        ));
    }

    let forbidden_prelude = [
        "GpuiOverlayAdapterConfig",
        "GpuiOverlayState",
        "TextInputController",
        "VirtualizedListGpuiExt",
        "TableGlobalFilter",
        "TablePredicateFilter",
        "TableFacetedFilter",
        "TableColumnVisibility",
        "TableRangeFilter",
        "TableToolbar",
        "ToolbarItem",
        "SidebarItem",
        "ListboxOption",
    ];
    reject_tokens(
        "crates/ui_components",
        "prelude.rs",
        &prelude_exports,
        &forbidden_prelude,
        "component prelude must not export adapter-only, recipe, or internal anatomy surfaces",
        failures,
    );

    let lib_source = read_to_string(&source_dir.join("lib.rs"), failures).unwrap_or_default();
    let gpui_adapter_source = public_module_source(&lib_source, "gpui_adapter").unwrap_or("");
    for required in [
        "TextInputController",
        "VirtualizedListGpuiExt",
        "UiA11yElementExt",
    ] {
        if !source_contains_identifier(gpui_adapter_source, required) {
            failures.push(format!(
                "crates/ui_components/src/lib.rs: gpui_adapter should export adapter helper `{required}`"
            ));
        }
    }
}

fn scan_ui_core_public_api(root: &Path, failures: &mut Vec<String>) {
    let source_dir = root.join("crates/ui_core/src");
    let prelude_exports = default_reexport_tokens(&source_dir, "prelude.rs", failures);
    let expected = ui_core_prelude_allowlist();
    let extra = prelude_exports
        .difference(&expected)
        .cloned()
        .collect::<Vec<_>>();
    let missing = expected
        .difference(&prelude_exports)
        .cloned()
        .collect::<Vec<_>>();
    if !extra.is_empty() || !missing.is_empty() {
        failures.push(format!(
            "crates/ui_core/src/prelude.rs: prelude allowlist drifted; extra [{}], missing [{}]",
            extra.join(", "),
            missing.join(", ")
        ));
    }
}

fn reject_tokens(
    crate_label: &str,
    file_name: &str,
    actual: &BTreeSet<String>,
    forbidden: &[&str],
    reason: &str,
    failures: &mut Vec<String>,
) {
    let leaked = forbidden
        .iter()
        .filter(|token| actual.contains(**token))
        .copied()
        .collect::<Vec<_>>();
    if leaked.is_empty() {
        return;
    }

    failures.push(format!(
        "{crate_label}/src/{file_name}: {reason}: {}",
        leaked.join(", ")
    ));
}

fn reject_public_module(path: &Path, module: &str, reason: &str, failures: &mut Vec<String>) {
    let Some(source) = read_to_string(path, failures) else {
        return;
    };
    let public_module = format!("pub mod {module};");
    for (line_index, line) in source.lines().enumerate() {
        if line.trim() == public_module {
            failures.push(format!("{}:{}: {reason}", path.display(), line_index + 1));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContractRow {
    default_export: bool,
}

fn component_contract_rows(
    source_dir: &Path,
    failures: &mut Vec<String>,
) -> BTreeMap<String, ContractRow> {
    let mut rows = BTreeMap::new();
    for (source_path, source) in contract_row_sources(source_dir, failures) {
        let (blocks, block_failures) = struct_literal_blocks(&source, "ComponentContractEntry");
        failures.extend(
            block_failures
                .into_iter()
                .map(|failure| format!("{source_path}: {failure}")),
        );

        for block in blocks {
            let Some(name) = string_field(block, "name") else {
                failures.push(format!("{source_path}: contract row missing `name`"));
                continue;
            };
            let Some(default_export) = bool_field(block, "default_export") else {
                failures.push(format!(
                    "{source_path}: contract row `{name}` missing `default_export`"
                ));
                continue;
            };
            if rows
                .insert(name.clone(), ContractRow { default_export })
                .is_some()
            {
                failures.push(format!("{source_path}: duplicate contract row `{name}`"));
            }
        }
    }
    rows
}

fn contract_row_sources(source_dir: &Path, failures: &mut Vec<String>) -> Vec<(String, String)> {
    let mut sources = Vec::new();
    let rows_path = source_dir.join("component_contract/rows.rs");
    if let Some(source) = read_to_string(&rows_path, failures) {
        sources.push((
            "crates/ui_components/src/component_contract/rows.rs".to_owned(),
            source,
        ));
    }

    let rows_dir = source_dir.join("component_contract/rows");
    if !rows_dir.is_dir() {
        return sources;
    }

    let mut row_files = match fs::read_dir(&rows_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .collect::<Vec<_>>(),
        Err(error) => {
            failures.push(format!(
                "crates/ui_components/src/component_contract/rows: failed to read directory: {error}"
            ));
            return sources;
        }
    };
    row_files.sort();

    for path in row_files {
        let label = path
            .strip_prefix(source_dir)
            .ok()
            .and_then(|relative| relative.to_str())
            .map(|relative| format!("crates/ui_components/src/{}", relative.replace('\\', "/")))
            .unwrap_or_else(|| path.display().to_string());
        if let Some(source) = read_to_string(&path, failures) {
            sources.push((label, source));
        }
    }

    sources
}

fn default_reexport_tokens(
    source_dir: &Path,
    file_name: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(source) = read_to_string(&source_dir.join(file_name), failures) else {
        return BTreeSet::new();
    };
    let source = if file_name == "lib.rs" {
        source_without_public_module(&source, "gpui_adapter")
    } else {
        source
    };
    reexport_tokens_from_source(&source, source_dir, failures)
}

fn reexport_tokens_from_source(
    source: &str,
    base_dir: &Path,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut exports = BTreeSet::new();
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
            collect_public_reexport_tokens(&statement, base_dir, failures, &mut exports);
            statement.clear();
            collecting = false;
        }
    }

    exports
}

fn collect_public_reexport_tokens(
    statement: &str,
    base_dir: &Path,
    failures: &mut Vec<String>,
    exports: &mut BTreeSet<String>,
) {
    let statement = statement.trim().trim_end_matches(';');
    let Some(rest) = statement.strip_prefix("pub use ") else {
        return;
    };
    if rest.contains("::*") {
        collect_curated_wildcard_reexport_tokens(rest, base_dir, failures, exports);
        return;
    }

    collect_reexport_expression_tokens(rest, exports);
}

fn collect_reexport_expression_tokens(expression: &str, exports: &mut BTreeSet<String>) {
    let expression = expression.trim();
    let Some(group_start) = expression.find("::{") else {
        collect_public_reexport_token(expression, exports);
        return;
    };
    let open_brace = group_start + 2;
    let Some(close_brace) = matching_brace(expression, open_brace) else {
        collect_public_reexport_token(expression, exports);
        return;
    };

    let prefix = expression[..group_start].trim();
    let group = &expression[open_brace + 1..close_brace];
    for item in split_top_level_commas(group) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let nested = if prefix.is_empty() {
            item.to_owned()
        } else {
            format!("{prefix}::{item}")
        };
        collect_reexport_expression_tokens(&nested, exports);
    }
}

fn split_top_level_commas(source: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (index, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(&source[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(&source[start..]);
    items
}

fn collect_curated_wildcard_reexport_tokens(
    rest: &str,
    base_dir: &Path,
    failures: &mut Vec<String>,
    exports: &mut BTreeSet<String>,
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
    let Some(source) = read_to_string(
        &base_dir
            .join("public_api")
            .join(format!("{relative_module_path}.rs")),
        failures,
    ) else {
        return;
    };
    exports.extend(reexport_tokens_from_source(&source, base_dir, failures));
}

fn collect_public_reexport_token(item: &str, exports: &mut BTreeSet<String>) {
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

fn public_signature_tokens(path: &Path, failures: &mut Vec<String>) -> BTreeSet<String> {
    let Some(source) = read_to_string(path, failures) else {
        return BTreeSet::new();
    };
    let mut tokens = BTreeSet::new();
    let mut statement = String::new();
    let mut collecting = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if !collecting
            && !(trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub const fn ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("pub type ")
                || trimmed.starts_with("pub trait "))
        {
            continue;
        }

        if !collecting {
            statement.clear();
            collecting = true;
        } else {
            statement.push(' ');
        }
        statement.push_str(trimmed);

        if trimmed.ends_with(';') || trimmed.ends_with('{') {
            tokens.extend(identifier_tokens(&statement));
            statement.clear();
            collecting = false;
        }
    }

    tokens
}

fn identifier_tokens(source: &str) -> BTreeSet<String> {
    source
        .split(|character: char| character != '_' && !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .filter(|part| {
            !matches!(
                *part,
                "pub"
                    | "use"
                    | "crate"
                    | "self"
                    | "super"
                    | "fn"
                    | "const"
                    | "struct"
                    | "enum"
                    | "type"
                    | "trait"
                    | "where"
                    | "impl"
                    | "for"
                    | "mut"
                    | "ref"
                    | "async"
                    | "unsafe"
                    | "extern"
                    | "dyn"
                    | "move"
                    | "as"
            )
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn source_contains_identifier(source: &str, token: &str) -> bool {
    source
        .split(|character: char| character != '_' && !character.is_ascii_alphanumeric())
        .any(|part| part == token)
}

fn read_to_string(path: &Path, failures: &mut Vec<String>) -> Option<String> {
    fs::read_to_string(path).map_or_else(
        |error| {
            failures.push(format!("{}: failed to read file: {error}", path.display()));
            None
        },
        Some,
    )
}

fn struct_literal_blocks<'a>(source: &'a str, type_name: &str) -> (Vec<&'a str>, Vec<String>) {
    let mut blocks = Vec::new();
    let mut failures = Vec::new();
    let mut search_from = 0usize;

    while let Some(relative_start) = source[search_from..].find(type_name) {
        let start = search_from + relative_start;
        let prefix = source[..start]
            .rsplit_once('\n')
            .map(|(_, line)| line)
            .unwrap_or("");
        let trimmed_prefix = prefix.trim_start();
        if trimmed_prefix.starts_with("//") || trimmed_prefix.starts_with("//!") {
            search_from = start + type_name.len();
            continue;
        }

        let Some(open_brace) = source[start..].find('{').map(|offset| start + offset) else {
            failures.push(format!("{type_name} literal is missing `{{`"));
            break;
        };
        let Some(close_brace) = matching_brace(source, open_brace) else {
            failures.push(format!("{type_name} literal is missing matching `}}`"));
            break;
        };
        blocks.push(&source[open_brace + 1..close_brace]);
        search_from = close_brace + 1;
    }

    (blocks, failures)
}

fn string_field(block: &str, field: &str) -> Option<String> {
    let rest = field_tail(block, field)?;
    quoted_value(rest.lines().next().unwrap_or_default())
}

fn bool_field(block: &str, field: &str) -> Option<bool> {
    let rest = field_tail(block, field)?.trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn field_tail<'a>(block: &'a str, field: &str) -> Option<&'a str> {
    let marker = format!("{field}:");
    let start = block.find(&marker)? + marker.len();
    Some(&block[start..])
}

fn quoted_value(source: &str) -> Option<String> {
    let start = source.find('"')? + 1;
    let end = source[start..].find('"').map(|offset| start + offset)?;
    Some(source[start..end].to_string())
}

fn source_without_public_module(source: &str, module_name: &str) -> String {
    let Some((module_start, close_brace)) = public_module_bounds(source, module_name) else {
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

fn ui_components_prelude_helper_allowlist() -> BTreeSet<&'static str> {
    [
        "ActiveDescendant",
        "CollectionPosition",
        "ControllableState",
        "Sizable",
        "Size",
        "ThemeTokens",
        "UiA11yElementExt",
    ]
    .into_iter()
    .collect()
}

fn ui_core_prelude_allowlist() -> BTreeSet<String> {
    [
        "AccessibleAction",
        "ActiveDescendant",
        "AdaptiveQuerySource",
        "CollectionPosition",
        "ControllableState",
        "Density",
        "DeviceAdaptiveClass",
        "DeviceAdaptivePolicy",
        "DeviceAdaptiveSnapshot",
        "DeviceShellMode",
        "DeviceShellSwitchPolicy",
        "DismissReason",
        "EscapeKeyPolicy",
        "EscapeKeyResolution",
        "FocusRestoreIntent",
        "FocusRestoreResolution",
        "FocusTargetId",
        "InitialFocusIntent",
        "Orientation",
        "OutsidePressOutcome",
        "OutsidePressPolicy",
        "OutsidePressResolution",
        "OverlayAnchorInput",
        "OverlayEdges",
        "OverlayFocusTarget",
        "OverlayLayer",
        "OverlayLayerId",
        "OverlayLayerKind",
        "OverlayLayerPolicy",
        "OverlayLayerState",
        "OverlayPlacementAlignment",
        "OverlayPlacementInput",
        "OverlayPlacementSide",
        "OverlayPresence",
        "OverlayResolvedState",
        "OverlaySize",
        "PanelAdaptiveClass",
        "PanelAdaptivePolicy",
        "Rect",
        "Role",
        "Sizable",
        "Size",
        "ThemeTokens",
        "Toggled",
        "TokenKey",
        "UiEdges",
        "UiPoint",
        "UiPx",
        "UiRect",
        "UiSize",
        "anchor_rect_from_point",
        "device_adaptive_class",
        "device_adaptive_snapshot",
        "device_shell_mode",
        "inset_rect",
        "outer_bounds_with_window_margin",
        "panel_adaptive_class",
        "prefer_visual_bounds",
        "rect",
        "resolve_focus_restore",
        "resolve_outside_press",
        "semantic",
        "ui_edges",
        "ui_point",
        "ui_px",
        "ui_rect",
        "ui_size",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reexport_parser_follows_curated_public_api_wildcards() {
        let root = temp_root("wildcard");
        let public_api = root.join("public_api");
        fs::create_dir_all(&public_api).unwrap();
        fs::write(root.join("lib.rs"), "pub use public_api::default::*;\n").unwrap();
        fs::write(
            public_api.join("default.rs"),
            "pub use crate::button::{Button, ButtonState as RenamedState};\n",
        )
        .unwrap();
        let mut failures = Vec::new();
        let tokens = default_reexport_tokens(&root, "lib.rs", &mut failures);
        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            tokens,
            ["Button".to_owned(), "RenamedState".to_owned()]
                .into_iter()
                .collect()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn struct_literal_parser_reads_component_contract_entries() {
        let source = r#"
            ComponentContractEntry {
                name: "Button",
                default_export: true,
            },
            ComponentContractEntry {
                name: "TextInputController",
                default_export: false,
            },
        "#;
        let (blocks, failures) = struct_literal_blocks(source, "ComponentContractEntry");
        assert!(failures.is_empty(), "{failures:?}");
        let values = blocks
            .into_iter()
            .map(|block| {
                (
                    string_field(block, "name").unwrap(),
                    bool_field(block, "default_export").unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![
                ("Button".to_owned(), true),
                ("TextInputController".to_owned(), false)
            ]
        );
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "open_gpui_public_api_snapshot_test_{label}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
