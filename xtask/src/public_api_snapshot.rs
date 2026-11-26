use std::{collections::BTreeSet, fs, path::Path};

pub(crate) fn scan_public_api(root: &Path, args: &[String]) -> Result<(), ()> {
    for arg in args {
        if arg != "--check" {
            eprintln!("unknown scan-public-api argument: {arg}");
            return Err(());
        }
    }

    println!("==> scan public docking API");

    let mut failures = Vec::new();
    scan_docking_public_api(root, &mut failures);
    if failures.is_empty() {
        println!("public docking API scan passed");
        Ok(())
    } else {
        eprintln!("public docking API scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
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
        let Some(source) = read_to_string(&source_dir.join(file_name), failures) else {
            continue;
        };
        let exports = reexport_tokens_from_source(&source);
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
        "legacy",
        "the displaced docking engine must stay out of the crate",
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
    for file_name in ["adapter/host.rs", "adapter/host_native.rs"] {
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

fn reexport_tokens_from_source(source: &str) -> BTreeSet<String> {
    let mut exports = BTreeSet::new();
    let mut statement = String::new();
    let mut collecting = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if collecting {
            statement.push(' ');
            statement.push_str(trimmed);
        } else if let Some(expression) = trimmed.strip_prefix("pub use ") {
            statement.clear();
            statement.push_str(expression);
            collecting = true;
        }

        if collecting && trimmed.ends_with(';') {
            collect_reexport_expression_tokens(statement.trim_end_matches(';'), &mut exports);
            statement.clear();
            collecting = false;
        }
    }

    exports
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

fn read_to_string(path: &Path, failures: &mut Vec<String>) -> Option<String> {
    fs::read_to_string(path).map_or_else(
        |error| {
            failures.push(format!("{}: failed to read file: {error}", path.display()));
            None
        },
        Some,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reexport_parser_reads_grouped_docking_exports() {
        let tokens = reexport_tokens_from_source(
            "pub use dockspace::{model::{DockAnchor, DockPlacement}, policy::DockPolicy};\n",
        );
        assert_eq!(
            tokens,
            ["DockAnchor", "DockPlacement", "DockPolicy"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        );
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
