use std::{collections::BTreeSet, fs, path::Path};

use open_gpui_ui_components::component_contract::common_public_exports;

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
    for surface in public_api_surfaces() {
        let start = failures.len();
        (surface.scan)(root, &mut failures);
        for failure in &mut failures[start..] {
            *failure = format!("{}: {failure}", surface.name);
        }
    }
    failures
}

#[derive(Clone, Copy, Debug)]
struct PublicApiSurface {
    name: &'static str,
    scan: fn(&Path, &mut Vec<String>),
}

fn public_api_surfaces() -> &'static [PublicApiSurface] {
    &[
        PublicApiSurface {
            name: "devtools",
            scan: scan_devtools_public_api,
        },
        PublicApiSurface {
            name: "docking",
            scan: scan_docking_public_api,
        },
        PublicApiSurface {
            name: "motion",
            scan: scan_motion_public_api,
        },
        PublicApiSurface {
            name: "ui-components",
            scan: scan_ui_components_public_api,
        },
        PublicApiSurface {
            name: "ui-core",
            scan: scan_ui_core_public_api,
        },
        PublicApiSurface {
            name: "canvas",
            scan: scan_canvas_public_api,
        },
    ]
}

fn scan_devtools_public_api(root: &Path, failures: &mut Vec<String>) {
    let source_dir = root.join("crates/devtools/src");
    let root_exports = root_reexport_tokens(&source_dir, "lib.rs", failures);
    let expected_exports = devtools_root_export_allowlist();
    let extra = root_exports
        .difference(&expected_exports)
        .cloned()
        .collect::<Vec<_>>();
    let missing = expected_exports
        .difference(&root_exports)
        .cloned()
        .collect::<Vec<_>>();
    if !extra.is_empty() || !missing.is_empty() {
        failures.push(format!(
            "crates/devtools/src/lib.rs: root export allowlist drifted; extra [{}], missing [{}]",
            extra.join(", "),
            missing.join(", ")
        ));
    }

    let Some(lib_source) = read_to_string(&source_dir.join("lib.rs"), failures) else {
        return;
    };
    let public_modules = public_module_declarations(&lib_source);
    let expected_modules = [
        "adapters",
        "command",
        "docking",
        "form",
        "gpui",
        "layout",
        "motion",
        "resource",
        "timeline",
        "ui_components",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect::<BTreeSet<_>>();
    let extra_modules = public_modules
        .difference(&expected_modules)
        .cloned()
        .collect::<Vec<_>>();
    let missing_modules = expected_modules
        .difference(&public_modules)
        .cloned()
        .collect::<Vec<_>>();
    if !extra_modules.is_empty() || !missing_modules.is_empty() {
        failures.push(format!(
            "crates/devtools/src/lib.rs: public module allowlist drifted; extra [{}], missing [{}]",
            extra_modules.join(", "),
            missing_modules.join(", ")
        ));
    }

    let ui_components_path = source_dir.join("ui_components.rs");
    let ui_components_items = top_level_public_item_names(&ui_components_path, failures);
    let expected_ui_components_items = devtools_ui_components_public_item_allowlist();
    let extra_items = ui_components_items
        .difference(&expected_ui_components_items)
        .cloned()
        .collect::<Vec<_>>();
    let missing_items = expected_ui_components_items
        .difference(&ui_components_items)
        .cloned()
        .collect::<Vec<_>>();
    if !extra_items.is_empty() || !missing_items.is_empty() {
        failures.push(format!(
            "crates/devtools/src/ui_components.rs: top-level public item allowlist drifted; extra [{}], missing [{}]",
            extra_items.join(", "),
            missing_items.join(", ")
        ));
    }

    reject_public_reexport_wildcards(
        &source_dir.join("lib.rs"),
        "devtools root public re-exports must stay explicit so artifact/report/session APIs are intentional",
        failures,
    );
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
    let advanced_exports = default_reexport_tokens(&source_dir, "advanced.rs", failures);
    let forbidden_root = [
        "MotionExecutionPlan",
        "MotionExecutionState",
        "MotionFrameHost",
        "MotionFrameHostResetReason",
        "MotionFrameHostSample",
        "MotionFrameHostUpdate",
        "MotionModel",
        "MotionPreset",
        "MotionPolicyInput",
        "MotionPreviewTargetPolicy",
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
        "validate_motion_policy",
    ];

    reject_tokens(
        "crates/motion",
        "lib.rs",
        &root_exports,
        &forbidden_root,
        "motion root must favor facade types over low-level execution/model internals",
        failures,
    );
    reject_public_module(
        &source_dir.join("lib.rs"),
        "policy",
        "low-level policy input validation must stay behind the advanced tier",
        failures,
    );

    let forbidden_advanced_lifecycle = ["MotionSpring", "MotionTimeline", "MotionTimelineSample"];
    reject_tokens(
        "crates/motion",
        "advanced.rs",
        &advanced_exports,
        &forbidden_advanced_lifecycle,
        "advanced surface must not export Instant-owning lifecycle types",
        failures,
    );

    for file_name in [
        "controller.rs",
        "frame_host.rs",
        "runtime.rs",
        "sequence.rs",
        "spring.rs",
        "transition.rs",
    ] {
        let tokens = public_signature_tokens(&source_dir.join(file_name), failures);
        reject_tokens(
            "crates/motion",
            file_name,
            &tokens,
            &["Instant"],
            "public motion lifecycle signatures must use elapsed time instead of Instant",
            failures,
        );
    }

    let transition_tokens = public_signature_tokens(&source_dir.join("transition.rs"), failures);
    reject_tokens(
        "crates/motion",
        "transition.rs",
        &transition_tokens,
        &[
            "MotionExecutionPlan",
            "MotionModel",
            "MotionPolicyInput",
            "MotionSpec",
            "MotionSpringSpec",
        ],
        "motion root facade signatures must hide low-level execution/model types",
        failures,
    );

    let sequence_tokens = public_signature_tokens(&source_dir.join("sequence.rs"), failures);
    reject_tokens(
        "crates/motion",
        "sequence.rs",
        &sequence_tokens,
        &[
            "MotionExecutionPlan",
            "MotionModel",
            "MotionPolicyInput",
            "MotionSpec",
        ],
        "root sequence facade signatures must accept facade transitions instead of low-level model types",
        failures,
    );
}

fn scan_ui_components_public_api(_root: &Path, failures: &mut Vec<String>) {
    failures.extend(crate::ui_contract::ui_component_public_export_failures());
    let common_exports = common_public_exports()
        .map(|export| export.name().to_owned())
        .collect::<BTreeSet<_>>();

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
        &common_exports,
        &forbidden_prelude,
        "component prelude must not export adapter-only, recipe, or internal anatomy surfaces",
        failures,
    );
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

fn scan_canvas_public_api(root: &Path, failures: &mut Vec<String>) {
    let source_dir = root.join("crates/canvas/src");
    let root_exports = root_reexport_tokens(&source_dir, "lib.rs", failures);

    require_tokens(
        "crates/canvas",
        "lib.rs",
        &root_exports,
        &[
            "CanvasDocument",
            "CanvasDocumentBuilder",
            "CanvasSnapshot",
            "CanvasStore",
            "CanvasKindRegistry",
            "CanvasViewport",
            "JsonCanvas",
        ],
        "canvas root must keep common document/editor/view imports available",
        failures,
    );

    reject_tokens(
        "crates/canvas",
        "lib.rs",
        &root_exports,
        &[
            "CanvasPaintFrame",
            "CanvasPaintModel",
            "CanvasPersistenceStore",
            "CanvasJsonPersistenceCodec",
            "CanvasGraphIndex",
            "CanvasGeometryFacts",
            "SpatialIndex",
        ],
        "canvas root must keep adapter, persistence, and advanced APIs behind explicit tiers",
        failures,
    );

    let root_source = fs::read_to_string(source_dir.join("lib.rs")).unwrap_or_else(|error| {
        failures.push(format!(
            "crates/canvas/lib.rs could not be read while checking tiers: {error}"
        ));
        String::new()
    });
    for module in ["adapter", "advanced", "persistence"] {
        if !root_source
            .lines()
            .any(|line| line.trim_start().starts_with(&format!("pub mod {module}")))
        {
            failures.push(format!(
                "crates/canvas/lib.rs missing public `{module}` API tier"
            ));
        }
    }

    reject_public_reexport_wildcards(
        &source_dir.join("lib.rs"),
        "canvas root public re-exports must stay explicit so API tiers can be scanned",
        failures,
    );
}

fn require_tokens(
    crate_label: &str,
    file_name: &str,
    actual: &BTreeSet<String>,
    required: &[&str],
    reason: &str,
    failures: &mut Vec<String>,
) {
    let missing = required
        .iter()
        .filter(|token| !actual.contains(**token))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return;
    }

    failures.push(format!(
        "{crate_label}/src/{file_name}: {reason}: missing {}",
        missing.join(", ")
    ));
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

fn reject_public_reexport_wildcards(path: &Path, reason: &str, failures: &mut Vec<String>) {
    let Some(source) = read_to_string(path, failures) else {
        return;
    };

    for (line_index, line) in source.lines().enumerate() {
        if line.contains("pub use ") && line.contains("::*") {
            failures.push(format!("{}:{}: {reason}", path.display(), line_index + 1));
        }
    }
}

fn reject_public_module(path: &Path, module: &str, reason: &str, failures: &mut Vec<String>) {
    let Some(source) = read_to_string(path, failures) else {
        return;
    };
    for (line_index, line) in source.lines().enumerate() {
        if is_public_module_declaration(line.trim(), module) {
            failures.push(format!("{}:{}: {reason}", path.display(), line_index + 1));
        }
    }
}

fn is_public_module_declaration(line: &str, module: &str) -> bool {
    let Some(rest) = line.strip_prefix("pub mod ") else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(module) else {
        return false;
    };
    matches!(rest.trim_start().chars().next(), Some(';') | Some('{'))
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

fn root_reexport_tokens(
    source_dir: &Path,
    file_name: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(source) = read_to_string(&source_dir.join(file_name), failures) else {
        return BTreeSet::new();
    };
    reexport_tokens_from_root_source(&source, source_dir, failures)
}

fn reexport_tokens_from_root_source(
    source: &str,
    base_dir: &Path,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut exports = BTreeSet::new();
    let mut statement = String::new();
    let mut collecting = false;
    let mut brace_depth = 0usize;

    for line in source.lines() {
        let trimmed = line.trim();
        if collecting {
            statement.push(' ');
            statement.push_str(trimmed);
        } else if brace_depth == 0 && trimmed.starts_with("pub use ") {
            statement.clear();
            statement.push_str(trimmed);
            collecting = true;
        }

        if collecting && trimmed.ends_with(';') {
            collect_public_reexport_tokens(&statement, base_dir, failures, &mut exports);
            statement.clear();
            collecting = false;
        }

        let opens = trimmed
            .chars()
            .filter(|character| *character == '{')
            .count();
        let closes = trimmed
            .chars()
            .filter(|character| *character == '}')
            .count();
        brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
    }

    exports
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

fn public_module_declarations(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("pub mod ")?;
            let name = rest
                .split(|character: char| {
                    character == ';' || character == '{' || character.is_whitespace()
                })
                .next()
                .unwrap_or_default();
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect()
}

fn top_level_public_item_names(path: &Path, failures: &mut Vec<String>) -> BTreeSet<String> {
    let Some(source) = read_to_string(path, failures) else {
        return BTreeSet::new();
    };
    let syntax = match syn::parse_file(&source) {
        Ok(syntax) => syntax,
        Err(error) => {
            failures.push(format!(
                "{}: failed to parse Rust source while scanning top-level public items: {error}",
                path.display()
            ));
            return BTreeSet::new();
        }
    };
    let mut items = BTreeSet::new();

    for item in &syntax.items {
        collect_top_level_public_item(item, path, &mut items, failures);
    }

    items
}

fn collect_top_level_public_item(
    item: &syn::Item,
    path: &Path,
    items: &mut BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    let public_ident = match item {
        syn::Item::Const(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::Enum(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::ExternCrate(item) => is_public(&item.vis).then_some(
            item.rename
                .as_ref()
                .map(|(_, rename)| rename)
                .unwrap_or(&item.ident),
        ),
        syn::Item::Fn(item) => is_public(&item.vis).then_some(&item.sig.ident),
        syn::Item::ForeignMod(item) => {
            for foreign_item in &item.items {
                collect_public_foreign_item(foreign_item, path, items, failures);
            }
            None
        }
        syn::Item::Impl(_) => None,
        syn::Item::Mod(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::Static(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::Struct(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::Trait(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::TraitAlias(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::Type(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::Union(item) => is_public(&item.vis).then_some(&item.ident),
        syn::Item::Use(item) => {
            if is_public(&item.vis) {
                collect_public_use_tree(&item.tree, None, path, items, failures);
            }
            None
        }
        syn::Item::Macro(item) => {
            let macro_name = macro_path_name(&item.mac.path);
            failures.push(format!(
                "{}: top-level item macro `{macro_name}!` may generate public API that the scanner cannot determine",
                path.display()
            ));
            None
        }
        syn::Item::Verbatim(_) => {
            failures.push(format!(
                "{}: top-level verbatim Rust syntax may define public API that the scanner cannot determine",
                path.display()
            ));
            None
        }
        _ => {
            failures.push(format!(
                "{}: unsupported top-level Rust item may define public API that the scanner cannot determine",
                path.display()
            ));
            None
        }
    };

    if let Some(ident) = public_ident {
        insert_exported_ident(ident, items);
    }
}

fn collect_public_foreign_item(
    item: &syn::ForeignItem,
    path: &Path,
    items: &mut BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    let public_ident = match item {
        syn::ForeignItem::Fn(item) => is_public(&item.vis).then_some(&item.sig.ident),
        syn::ForeignItem::Static(item) => is_public(&item.vis).then_some(&item.ident),
        syn::ForeignItem::Type(item) => is_public(&item.vis).then_some(&item.ident),
        syn::ForeignItem::Macro(item) => {
            let macro_name = macro_path_name(&item.mac.path);
            failures.push(format!(
                "{}: foreign item macro `{macro_name}!` may generate public API that the scanner cannot determine",
                path.display()
            ));
            None
        }
        syn::ForeignItem::Verbatim(_) => {
            failures.push(format!(
                "{}: verbatim foreign-item syntax may define public API that the scanner cannot determine",
                path.display()
            ));
            None
        }
        _ => {
            failures.push(format!(
                "{}: unsupported foreign item may define public API that the scanner cannot determine",
                path.display()
            ));
            None
        }
    };

    if let Some(ident) = public_ident {
        insert_exported_ident(ident, items);
    }
}

fn macro_path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn collect_public_use_tree(
    tree: &syn::UseTree,
    parent: Option<&syn::Ident>,
    path: &Path,
    items: &mut BTreeSet<String>,
    failures: &mut Vec<String>,
) {
    match tree {
        syn::UseTree::Path(use_path) => {
            collect_public_use_tree(&use_path.tree, Some(&use_path.ident), path, items, failures);
        }
        syn::UseTree::Name(use_name) if use_name.ident == "self" => {
            if let Some(parent) = parent {
                insert_exported_ident(parent, items);
            } else {
                failures.push(format!(
                    "{}: top-level public `use self` has no exported name the scanner can determine",
                    path.display()
                ));
            }
        }
        syn::UseTree::Name(use_name) => insert_exported_ident(&use_name.ident, items),
        syn::UseTree::Rename(use_rename) => insert_exported_ident(&use_rename.rename, items),
        syn::UseTree::Glob(_) => failures.push(format!(
            "{}: top-level public glob re-export cannot determine exported names; use explicit names instead",
            path.display()
        )),
        syn::UseTree::Group(use_group) => {
            for tree in &use_group.items {
                collect_public_use_tree(tree, parent, path, items, failures);
            }
        }
    }
}

fn insert_exported_ident(ident: &syn::Ident, items: &mut BTreeSet<String>) {
    let ident = ident.to_string();
    if ident != "_" {
        items.insert(ident);
    }
}

fn is_public(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Public(_))
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

fn read_to_string(path: &Path, failures: &mut Vec<String>) -> Option<String> {
    fs::read_to_string(path).map_or_else(
        |error| {
            failures.push(format!("{}: failed to read file: {error}", path.display()));
            None
        },
        Some,
    )
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

fn devtools_root_export_allowlist() -> BTreeSet<String> {
    [
        "CaptureProvider",
        "DEFAULT_DEVTOOLS_EVENT_LIMIT",
        "DEFAULT_DEVTOOLS_EVENT_SCOPE_ID",
        "DEFAULT_DEVTOOLS_EVENT_SCOPE_LABEL",
        "DEFAULT_DEVTOOLS_SESSION_HISTORY_LIMIT",
        "DEFAULT_TIMELINE_EVENT_LIMIT",
        "DEVTOOLS_ARTIFACT_RECORD_SCHEMA_VERSION",
        "DEVTOOLS_REPORT_SCHEMA_VERSION",
        "DEVTOOLS_SESSION_PROTOCOL_VERSION",
        "DEVTOOLS_SESSION_SCHEMA_VERSION",
        "DevtoolsArtifact",
        "DevtoolsArtifactFileMode",
        "DevtoolsArtifactFileSink",
        "DevtoolsArtifactJsonlSink",
        "DevtoolsArtifactKind",
        "DevtoolsArtifactMetadata",
        "DevtoolsArtifactRecord",
        "DevtoolsArtifactSink",
        "DevtoolsArtifactWriteError",
        "DevtoolsCapture",
        "DevtoolsCaptureDiff",
        "DevtoolsCaptureProvider",
        "DevtoolsDiffKind",
        "DevtoolsDiffRow",
        "DevtoolsDiffStatus",
        "DevtoolsDiffSummary",
        "DevtoolsDomainId",
        "DevtoolsDomainKind",
        "DevtoolsDomainRow",
        "DevtoolsDomainSnapshot",
        "DevtoolsEventBatch",
        "DevtoolsEventIdentity",
        "DevtoolsEventKind",
        "DevtoolsEventRecord",
        "DevtoolsEventRecorder",
        "DevtoolsEventRow",
        "DevtoolsInspector",
        "DevtoolsInspectorCaptureExport",
        "DevtoolsInspectorController",
        "DevtoolsInspectorDetail",
        "DevtoolsInspectorDetailKind",
        "DevtoolsInspectorError",
        "DevtoolsInspectorJsonAction",
        "DevtoolsInspectorSessionFrameSummary",
        "DevtoolsInspectorState",
        "DevtoolsProbe",
        "DevtoolsRegistry",
        "DevtoolsRegistryError",
        "DevtoolsReport",
        "DevtoolsReportFinding",
        "DevtoolsReportSeverity",
        "DevtoolsReportSource",
        "DevtoolsReportSourceKind",
        "DevtoolsReportSummary",
        "DevtoolsSession",
        "DevtoolsSessionConnectionState",
        "DevtoolsSessionError",
        "DevtoolsSessionExport",
        "DevtoolsSessionFrame",
        "DevtoolsSessionImportError",
        "DevtoolsSessionImportLimits",
        "DevtoolsSnapshotCategory",
        "DevtoolsSnapshotCategorySummary",
        "DevtoolsSnapshotRow",
        "DevtoolsTargetId",
        "DevtoolsTargetKind",
        "DevtoolsTargetRow",
        "DevtoolsTargetSnapshot",
        "DevtoolsTargetTree",
        "DevtoolsWorkbench",
        "DevtoolsWorkbenchDiffState",
        "DevtoolsWorkbenchRefreshStatus",
        "GpuiRuntimeFocusSnapshot",
        "GpuiRuntimeFrameSnapshot",
        "GpuiRuntimeInputSnapshot",
        "GpuiRuntimePointSnapshot",
        "GpuiRuntimeRectSnapshot",
        "GpuiRuntimeScrollSnapshot",
        "GpuiRuntimeSizeSnapshot",
        "GpuiRuntimeSnapshot",
        "GpuiRuntimeWindowSnapshot",
        "LayoutBoundsSnapshot",
        "LayoutNodeSnapshot",
        "LayoutPointSnapshot",
        "LayoutSizeSnapshot",
        "LayoutSnapshot",
        "ProbeId",
        "ProbeSnapshotError",
        "SnapshotCollection",
        "SnapshotDiagnostic",
        "SnapshotEnvelope",
        "SnapshotKind",
        "SnapshotNode",
        "SnapshotProbe",
        "SnapshotProbeSnapshot",
        "SnapshotRedactionSummary",
        "SnapshotTree",
        "TimelineEventSnapshot",
        "TimelineSnapshot",
        "gpui_runtime_capture",
        "gpui_runtime_capture_provider",
        "gpui_runtime_probe_snapshot",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}

fn devtools_ui_components_public_item_allowlist() -> BTreeSet<String> {
    [
        "ComponentSemanticIdentity",
        "OpaqueSemanticNodeId",
        "ResolvedSemanticNode",
        "resolved_semantics_probe_snapshot",
        "theme_probe_snapshot",
        "window_overlay_probe_snapshot",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}

fn ui_core_prelude_allowlist() -> BTreeSet<String> {
    [
        "AccessibleAction",
        "AccessibleTextPosition",
        "AccessibleTextSelection",
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
        "FocusResolution",
        "FocusRestoreInput",
        "FocusRestoreIntent",
        "FocusScopeId",
        "FocusScopeMode",
        "FocusScopePolicy",
        "FocusTargetAvailability",
        "FocusTargetCandidate",
        "FocusTargetId",
        "InitialFocusIntent",
        "Orientation",
        "OutsidePressOutcome",
        "OutsidePressParticipation",
        "OutsidePressPolicy",
        "OutsidePressResolution",
        "OverlayAnchorInput",
        "OverlayEdges",
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
        "SemanticDescriptor",
        "Sizable",
        "Size",
        "SizeScale",
        "SortDirection",
        "ThemeDesignScales",
        "ThemeElevationLayer",
        "ThemeElevationScale",
        "ThemeRadiusScale",
        "ThemeSpacingScale",
        "ThemeTokens",
        "ThemeTypographyScale",
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
        "resolve_focus_scope_restore",
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
    fn public_module_rejection_catches_file_and_inline_modules() {
        let root = temp_root("public_module_rejection");
        let lib = root.join("lib.rs");
        fs::write(
            &lib,
            r#"
                mod layout;
                pub(crate) mod layout;
                pub mod layout_extra;
                pub mod layout;
                pub mod policy {
                    pub struct Policy;
                }
            "#,
        )
        .unwrap();

        let mut failures = Vec::new();
        reject_public_module(&lib, "layout", "layout must stay private", &mut failures);
        reject_public_module(&lib, "policy", "policy must stay private", &mut failures);

        assert_eq!(failures.len(), 2, "{failures:?}");
        assert!(failures[0].contains("layout must stay private"));
        assert!(failures[1].contains("policy must stay private"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn public_api_surface_registry_includes_canvas() {
        let surface_names = public_api_surfaces()
            .iter()
            .map(|surface| surface.name)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            surface_names,
            [
                "canvas",
                "devtools",
                "docking",
                "motion",
                "ui-components",
                "ui-core"
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn top_level_public_item_scanner_ignores_braces_outside_rust_syntax() {
        let root = temp_root("top_level_public_item_braces");
        let source = root.join("items.rs");
        fs::write(
            &source,
            r####"
                const NORMAL_STRING: &str = "{";
                pub struct AfterNormalString;
                const NORMAL_STRING_RESET: &str = "}";

                const RAW_STRING: &str = r###"{"###;
                pub enum AfterRawString {}
                const RAW_STRING_RESET: &str = r###"}"###;

                // {
                pub const AFTER_LINE_COMMENT: usize = 1;
                // }

                /* { */
                pub type AfterBlockComment = ();
                /* } */
            "####,
        )
        .expect("brace scanner fixture should be writable");

        let mut failures = Vec::new();
        let items = top_level_public_item_names(&source, &mut failures);

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            items,
            [
                "AFTER_LINE_COMMENT",
                "AfterBlockComment",
                "AfterNormalString",
                "AfterRawString",
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn top_level_public_item_scanner_collects_supported_items_and_grouped_uses() {
        let root = temp_root("top_level_public_item_kinds");
        let source = root.join("items.rs");
        fs::write(
            &source,
            r#"
                #[doc = "a public function split across lines"]
                pub
                fn multiline_function() {}

                #[derive(Clone)]
                pub struct ExportedStruct;
                pub enum ExportedEnum { Variant }
                pub union ExportedUnion { value: u32 }
                pub trait ExportedTrait {}
                pub type ExportedType = u32;
                pub const EXPORTED_CONST: u32 = 1;
                pub static EXPORTED_STATIC: u32 = 1;
                pub mod exported_module {
                    pub struct NestedItem;
                }
                pub use crate::api::{
                    Direct,
                    Nested::{self, Original as Alias},
                    module as RenamedModule,
                };

                pub(crate) struct CrateVisibleOnly;
                impl ExportedStruct {
                    pub fn associated_function() {}
                }
            "#,
        )
        .expect("public item scanner fixture should be writable");

        let mut failures = Vec::new();
        let items = top_level_public_item_names(&source, &mut failures);

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            items,
            [
                "Alias",
                "Direct",
                "EXPORTED_CONST",
                "EXPORTED_STATIC",
                "ExportedEnum",
                "ExportedStruct",
                "ExportedTrait",
                "ExportedType",
                "ExportedUnion",
                "Nested",
                "RenamedModule",
                "exported_module",
                "multiline_function",
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn top_level_public_item_scanner_collects_extern_crate_and_foreign_exports() {
        let root = temp_root("top_level_public_item_extern_exports");
        let source = root.join("items.rs");
        fs::write(
            &source,
            r#"
                pub extern crate serde as public_serde;
                extern "C" {
                    pub fn public_foreign_function(value: i32) -> i32;
                    pub static PUBLIC_FOREIGN_STATIC: i32;
                    fn private_foreign_function();
                }
            "#,
        )
        .expect("extern export scanner fixture should be writable");

        let mut failures = Vec::new();
        let items = top_level_public_item_names(&source, &mut failures);

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            items,
            [
                "PUBLIC_FOREIGN_STATIC",
                "public_foreign_function",
                "public_serde",
            ]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn top_level_public_item_scanner_rejects_foreign_item_macros() {
        let root = temp_root("top_level_public_item_foreign_macro");
        let source = root.join("items.rs");
        fs::write(
            &source,
            r#"
                extern "C" {
                    generate_foreign_api!();
                }
            "#,
        )
        .expect("foreign macro scanner fixture should be writable");

        let mut failures = Vec::new();
        let items = top_level_public_item_names(&source, &mut failures);

        assert!(items.is_empty());
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(
            failures[0].contains("generate_foreign_api!"),
            "{failures:?}"
        );
        assert!(
            failures[0].contains("may generate public API"),
            "{failures:?}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn top_level_public_item_scanner_rejects_glob_reexports() {
        let root = temp_root("top_level_public_item_glob");
        let source = root.join("items.rs");
        fs::write(
            &source,
            "pub use crate::api::{Known, nested::*, Original as Alias};\n",
        )
        .expect("glob scanner fixture should be writable");

        let mut failures = Vec::new();
        let items = top_level_public_item_names(&source, &mut failures);

        assert_eq!(
            items,
            ["Alias".to_owned(), "Known".to_owned()]
                .into_iter()
                .collect()
        );
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("glob re-export"), "{failures:?}");
        assert!(
            failures[0].contains("cannot determine exported names"),
            "{failures:?}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn top_level_public_item_scanner_rejects_item_macros() {
        let root = temp_root("top_level_public_item_macros");
        let source = root.join("items.rs");
        fs::write(
            &source,
            r#"
                include!("generated_items.rs");
                generate_public_api!();
                pub struct KnownItem;
            "#,
        )
        .expect("macro scanner fixture should be writable");

        let mut failures = Vec::new();
        let items = top_level_public_item_names(&source, &mut failures);

        assert_eq!(items, ["KnownItem".to_owned()].into_iter().collect());
        assert_eq!(failures.len(), 2, "{failures:?}");
        assert!(
            failures.iter().any(|failure| failure.contains("include!")),
            "{failures:?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("generate_public_api!")),
            "{failures:?}"
        );
        assert!(
            failures
                .iter()
                .all(|failure| failure.contains("may generate public API")),
            "{failures:?}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn devtools_public_api_scan_tracks_artifact_pipeline_exports() {
        let root = temp_root("devtools_public_api");
        let devtools_src = root.join("crates/devtools/src");
        fs::create_dir_all(&devtools_src).unwrap();
        let ui_components_api = r#"
            pub fn theme_probe_snapshot() {}
            pub fn window_overlay_probe_snapshot() {}
            pub struct ComponentSemanticIdentity;
            pub struct OpaqueSemanticNodeId;
            pub struct ResolvedSemanticNode;
            pub fn resolved_semantics_probe_snapshot() {}

            impl ComponentSemanticIdentity {
                pub fn contract_id(&self) -> &'static str { "Button" }
            }

            pub(crate) struct CratePrivateSemanticNode;
            fn private_semantic_helper() {}
        "#;
        fs::write(devtools_src.join("ui_components.rs"), ui_components_api).unwrap();
        let modules = [
            "adapters",
            "command",
            "docking",
            "form",
            "gpui",
            "layout",
            "motion",
            "resource",
            "timeline",
            "ui_components",
        ]
        .into_iter()
        .map(|module| format!("pub mod {module};"))
        .collect::<Vec<_>>()
        .join("\n");
        let exports = devtools_root_export_allowlist()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        fs::write(
            devtools_src.join("lib.rs"),
            format!("{modules}\npub use api::{{{exports}}};\n"),
        )
        .unwrap();

        let mut failures = Vec::new();
        scan_devtools_public_api(&root, &mut failures);
        assert!(failures.is_empty(), "{failures:?}");

        fs::write(
            devtools_src.join("ui_components.rs"),
            ui_components_api.replace("pub struct ResolvedSemanticNode;\n", ""),
        )
        .unwrap();
        let mut failures = Vec::new();
        scan_devtools_public_api(&root, &mut failures);
        assert_eq!(
            failures,
            [
                "crates/devtools/src/ui_components.rs: top-level public item allowlist drifted; extra [], missing [ResolvedSemanticNode]"
            ]
        );

        fs::write(
            devtools_src.join("ui_components.rs"),
            format!("{ui_components_api}\npub struct LeakedSemanticPayload;\n"),
        )
        .unwrap();
        let mut failures = Vec::new();
        scan_devtools_public_api(&root, &mut failures);
        assert_eq!(
            failures,
            [
                "crates/devtools/src/ui_components.rs: top-level public item allowlist drifted; extra [LeakedSemanticPayload], missing []"
            ]
        );

        fs::write(
            devtools_src.join("lib.rs"),
            "pub mod adapters;\npub mod report_rules;\npub use api::{DevtoolsArtifact, DevtoolsReport, LeakedReportRule};\n",
        )
        .unwrap();

        let mut failures = Vec::new();
        scan_devtools_public_api(&root, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("root export allowlist drifted")),
            "{failures:?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("public module allowlist drifted")),
            "{failures:?}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn canvas_public_api_scan_keeps_common_root_imports_explicit() {
        let root = temp_root("canvas_public_api");
        let canvas_src = root.join("crates/canvas/src");
        fs::create_dir_all(&canvas_src).unwrap();
        fs::write(
            canvas_src.join("lib.rs"),
            r#"
                pub mod adapter {}
                pub mod advanced {}
                pub mod persistence {}
                pub use document::{CanvasDocument, CanvasDocumentBuilder, CanvasSnapshot};
                pub use store::{CanvasStore};
                pub use schema::{CanvasKindRegistry};
                pub use geometry::{CanvasViewport};
                pub use json_canvas::{JsonCanvas};
            "#,
        )
        .unwrap();

        let mut failures = Vec::new();
        scan_canvas_public_api(&root, &mut failures);
        assert!(failures.is_empty(), "{failures:?}");

        fs::write(
            canvas_src.join("lib.rs"),
            r#"
                pub mod adapter {}
                pub mod advanced {}
                pub mod persistence {}
                pub use document::*;
                pub use store::{CanvasStore};
            "#,
        )
        .unwrap();

        let mut failures = Vec::new();
        scan_canvas_public_api(&root, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("missing CanvasDocument")),
            "{failures:?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("re-exports must stay explicit")),
            "{failures:?}"
        );

        fs::write(
            canvas_src.join("lib.rs"),
            r#"
                pub mod adapter {}
                pub mod advanced {}
                pub mod persistence {}
                pub use document::{CanvasDocument, CanvasDocumentBuilder, CanvasSnapshot};
                pub use geometry::{CanvasViewport};
                pub use gpui::{CanvasPaintFrame};
                pub use json_canvas::{JsonCanvas};
                pub use schema::{CanvasKindRegistry};
                pub use store::{CanvasStore};
            "#,
        )
        .unwrap();

        let mut failures = Vec::new();
        scan_canvas_public_api(&root, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("adapter, persistence, and advanced APIs")),
            "{failures:?}"
        );

        let _ = fs::remove_dir_all(root);
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
