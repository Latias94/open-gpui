use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegistryEntry {
    name: String,
    docs_status: DocsStatus,
    docs_token: Option<String>,
    default_export: bool,
    source_home: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocsStatus {
    ComponentCatalog,
    ComponentContract,
    ComponentContractOrVerification,
    Verification,
}

struct Docs<'a> {
    component_contract: &'a str,
    verification: &'a str,
}

pub(crate) fn scan_ui_contract(root: &Path) -> Result<(), ()> {
    println!("==> scan UI contract");

    let failures = ui_contract_failures(root);
    if failures.is_empty() {
        println!("UI contract scan passed");
        Ok(())
    } else {
        eprintln!("UI contract scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn ui_contract_failures(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    let source_dir = root.join("crates/ui_components/src");
    let registry_path = source_dir.join("component_contract/rows.rs");
    let component_contract_docs_path = root.join("docs/ui/component-contract.md");
    let verification_docs_path = root.join("docs/verification.md");

    let registry_source = read_to_string(&registry_path, &mut failures);
    let component_contract_docs = read_to_string(&component_contract_docs_path, &mut failures);
    let verification_docs = read_to_string(&verification_docs_path, &mut failures);
    let root_exports = default_reexport_tokens(&source_dir, "lib.rs", &mut failures);
    let prelude_exports = default_reexport_tokens(&source_dir, "prelude.rs", &mut failures);

    let Some(registry_source) = registry_source else {
        return failures;
    };
    let Some(component_contract_docs) = component_contract_docs else {
        return failures;
    };
    let Some(verification_docs) = verification_docs else {
        return failures;
    };

    let (entries, parse_failures) = registry_entries_from_source(&registry_source);
    failures.extend(
        parse_failures.into_iter().map(|failure| {
            format!("crates/ui_components/src/component_contract/rows.rs: {failure}")
        }),
    );

    let docs = Docs {
        component_contract: &component_contract_docs,
        verification: &verification_docs,
    };
    failures.extend(audit_registry_entries(
        &entries,
        &root_exports,
        &prelude_exports,
        &docs,
        |entry| source_home_exists(&source_dir, entry),
        |name| removed_primitive_module_exists(&source_dir, name),
    ));

    failures
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

fn audit_registry_entries(
    entries: &[RegistryEntry],
    root_exports: &BTreeSet<String>,
    prelude_exports: &BTreeSet<String>,
    docs: &Docs<'_>,
    mut source_home_exists: impl FnMut(&RegistryEntry) -> bool,
    mut removed_primitive_exists: impl FnMut(&str) -> bool,
) -> Vec<String> {
    let mut failures = Vec::new();
    let mut owners = BTreeMap::new();

    for entry in entries {
        if let Some(previous) = owners.insert(entry.name.as_str(), entry.source_home.as_str()) {
            failures.push(format!(
                "crates/ui_components/src/component_contract/rows.rs: registry row `{}` is duplicated; previous source_home `{previous}`, duplicate source_home `{}`",
                entry.name, entry.source_home
            ));
        }

        if entry.default_export {
            if !root_exports.contains(&entry.name) {
                failures.push(format!(
                    "crates/ui_components/src/lib.rs: default-export registry row `{}` is missing from crate root exports; add it to crates/ui_components/src/public_api/default.rs or explicitly re-export it",
                    entry.name
                ));
            }
            if !prelude_exports.contains(&entry.name) {
                failures.push(format!(
                    "crates/ui_components/src/prelude.rs: default-export registry row `{}` is missing from prelude exports; add it to crates/ui_components/src/public_api/default.rs or explicitly re-export it",
                    entry.name
                ));
            }
        }

        if entry.source_home == "removed" {
            if removed_primitive_exists(&entry.name) {
                failures.push(format!(
                    "crates/ui_components/src/primitives/mod.rs: removed primitive `{}` reappeared; delete the compatibility module or update the registry ownership",
                    entry.name
                ));
            }
        } else if !source_home_exists(entry) {
            failures.push(format!(
                "crates/ui_components/src/component_contract/rows.rs: registry row `{}` source_home `{}` does not exist under crates/ui_components/src",
                entry.name, entry.source_home
            ));
        }

        if let Some(token) = &entry.docs_token {
            audit_docs_token(entry, token, docs, &mut failures);
        }
    }

    failures
}

fn audit_docs_token(
    entry: &RegistryEntry,
    token: &str,
    docs: &Docs<'_>,
    failures: &mut Vec<String>,
) {
    match entry.docs_status {
        DocsStatus::ComponentCatalog => {}
        DocsStatus::ComponentContract => {
            if !docs.component_contract.contains(token) {
                failures.push(format!(
                    "docs/ui/component-contract.md: missing docs token `{token}` for registry row `{}`",
                    entry.name
                ));
            }
        }
        DocsStatus::ComponentContractOrVerification => {
            if !docs.component_contract.contains(token) && !docs.verification.contains(token) {
                failures.push(format!(
                    "docs/ui/component-contract.md or docs/verification.md: missing docs token `{token}` for registry row `{}`",
                    entry.name
                ));
            }
        }
        DocsStatus::Verification => {
            if !docs.verification.contains(token)
                && !docs
                    .verification
                    .contains("primitive_deletion_target_inventory")
            {
                failures.push(format!(
                    "docs/verification.md: missing docs token `{token}` for registry row `{}`",
                    entry.name
                ));
            }
        }
    }
}

fn source_home_exists(source_dir: &Path, entry: &RegistryEntry) -> bool {
    if entry.source_home == "gpui_adapter" {
        let lib_rs = source_dir.join("lib.rs");
        return fs::read_to_string(lib_rs).is_ok_and(|source| {
            public_module_source(&source, "gpui_adapter")
                .is_some_and(|module_source| module_source.contains(&entry.name))
        });
    }

    let path = source_dir.join(&entry.source_home);
    path.is_file() || path.is_dir()
}

fn removed_primitive_module_exists(source_dir: &Path, name: &str) -> bool {
    let Some(module) = name.strip_prefix("primitives::") else {
        return false;
    };

    let module_file = source_dir.join("primitives").join(format!("{module}.rs"));
    if module_file.exists() {
        return true;
    }

    let mod_rs = source_dir.join("primitives/mod.rs");
    fs::read_to_string(mod_rs).is_ok_and(|source| {
        source
            .lines()
            .any(|line| line.trim() == format!("pub mod {module};"))
    })
}

fn registry_entries_from_source(source: &str) -> (Vec<RegistryEntry>, Vec<String>) {
    let mut entries = Vec::new();
    let mut failures = Vec::new();
    let mut search_from = 0;

    while let Some(relative_start) = source[search_from..].find("ComponentContractEntry") {
        let start = search_from + relative_start;
        let Some(open_brace) = source[start..].find('{').map(|offset| start + offset) else {
            failures.push("registry entry is missing `{`".to_string());
            break;
        };
        let Some(close_brace) = matching_brace(source, open_brace) else {
            failures.push("registry entry is missing matching `}`".to_string());
            break;
        };
        let block = &source[open_brace + 1..close_brace];

        match registry_entry_from_block(block) {
            Ok(entry) => entries.push(entry),
            Err(error) => failures.push(error),
        }

        search_from = close_brace + 1;
    }

    (entries, failures)
}

fn registry_entry_from_block(block: &str) -> Result<RegistryEntry, String> {
    let name = string_field(block, "name").ok_or("registry row missing `name`")?;
    let docs_status = docs_status_field(block)
        .ok_or_else(|| format!("registry row `{name}` missing or has unknown `docs_status`"))?;
    let docs_token = optional_string_field(block, "docs_token");
    let default_export = bool_field(block, "default_export")
        .ok_or_else(|| format!("registry row `{name}` missing `default_export`"))?;
    let source_home = string_field(block, "source_home")
        .ok_or_else(|| format!("registry row `{name}` missing `source_home`"))?;

    Ok(RegistryEntry {
        name,
        docs_status,
        docs_token,
        default_export,
        source_home,
    })
}

fn string_field(block: &str, field: &str) -> Option<String> {
    let rest = field_value(block, field)?;
    quoted_value(rest)
}

fn optional_string_field(block: &str, field: &str) -> Option<String> {
    let rest = field_value(block, field)?.trim_start();
    if rest.starts_with("None") {
        None
    } else {
        quoted_value(rest)
    }
}

fn bool_field(block: &str, field: &str) -> Option<bool> {
    let rest = field_value(block, field)?.trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn docs_status_field(block: &str) -> Option<DocsStatus> {
    let variant = enum_variant_field(block, "docs_status")?;
    match variant.as_str() {
        "ComponentCatalog" => Some(DocsStatus::ComponentCatalog),
        "ComponentContract" => Some(DocsStatus::ComponentContract),
        "ComponentContractOrVerification" => Some(DocsStatus::ComponentContractOrVerification),
        "Verification" => Some(DocsStatus::Verification),
        _ => None,
    }
}

fn enum_variant_field(block: &str, field: &str) -> Option<String> {
    let rest = field_value(block, field)?.trim_start();
    let variant_start = rest.rfind("::").map(|index| index + 2).unwrap_or(0);
    let variant = rest[variant_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    (!variant.is_empty()).then_some(variant)
}

fn field_value<'a>(block: &'a str, field: &str) -> Option<&'a str> {
    let marker = format!("{field}:");
    let start = block.find(&marker)? + marker.len();
    Some(block[start..].lines().next().unwrap_or_default())
}

fn quoted_value(source: &str) -> Option<String> {
    let start = source.find('"')? + 1;
    let end = source[start..].find('"').map(|offset| start + offset)?;
    Some(source[start..end].to_string())
}

fn default_reexport_tokens(
    source_dir: &Path,
    file_name: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let source_path = source_dir.join(file_name);
    let Some(source) = read_to_string(&source_path, failures) else {
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
    let source_path = base_dir
        .join("public_api")
        .join(format!("{relative_module_path}.rs"));
    let Some(source) = read_to_string(&source_path, failures) else {
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

fn public_module_source<'a>(source: &'a str, module_name: &str) -> Option<&'a str> {
    let (module_start, close_brace) = public_module_bounds(source, module_name)?;
    Some(&source[module_start..=close_brace])
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

    fn entry(
        name: &str,
        docs_status: DocsStatus,
        docs_token: Option<&str>,
        default_export: bool,
        source_home: &str,
    ) -> RegistryEntry {
        RegistryEntry {
            name: name.to_string(),
            docs_status,
            docs_token: docs_token.map(str::to_string),
            default_export,
            source_home: source_home.to_string(),
        }
    }

    fn docs<'a>(component_contract: &'a str, verification: &'a str) -> Docs<'a> {
        Docs {
            component_contract,
            verification,
        }
    }

    fn has_failure(failures: &[String], needle: &str) -> bool {
        failures.iter().any(|failure| failure.contains(needle))
    }

    #[test]
    fn registry_parser_reads_entry_fields() {
        let source = r#"
pub const COMPONENT_CONTRACT_REGISTRY: &[ComponentContractEntry] = &[
    ComponentContractEntry {
        name: "Button",
        owner: PublicSurfaceOwnerClass::OfficialComponent,
        family: Some("action"),
        gallery_status: SurfaceGalleryStatus::OfficialComponent,
        docs_status: SurfaceDocsStatus::ComponentContract,
        docs_token: Some("Button contract"),
        default_export: true,
        source_inputs: &["button.rs"],
        source_home: "button.rs",
    },
];
"#;

        let (entries, failures) = registry_entries_from_source(source);

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Button");
        assert_eq!(entries[0].docs_status, DocsStatus::ComponentContract);
        assert_eq!(entries[0].docs_token.as_deref(), Some("Button contract"));
        assert!(entries[0].default_export);
        assert_eq!(entries[0].source_home, "button.rs");
    }

    #[test]
    fn audit_reports_missing_default_export() {
        let entries = [entry(
            "Button",
            DocsStatus::ComponentCatalog,
            Some("Button"),
            true,
            "button.rs",
        )];
        let root_exports = BTreeSet::new();
        let prelude_exports = BTreeSet::from(["Button".to_string()]);

        let failures = audit_registry_entries(
            &entries,
            &root_exports,
            &prelude_exports,
            &docs("", ""),
            |entry| entry.source_home == "button.rs",
            |_| false,
        );

        assert!(has_failure(&failures, "crate root exports"));
        assert!(has_failure(&failures, "Button"));
    }

    #[test]
    fn audit_reports_missing_docs_token() {
        let entries = [entry(
            "Tooltip",
            DocsStatus::ComponentContract,
            Some("Tooltip contract"),
            false,
            "tooltip.rs",
        )];

        let failures = audit_registry_entries(
            &entries,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &docs("Button contract", ""),
            |entry| entry.source_home == "tooltip.rs",
            |_| false,
        );

        assert!(has_failure(&failures, "Tooltip contract"));
        assert!(has_failure(&failures, "docs/ui/component-contract.md"));
    }

    #[test]
    fn audit_reports_missing_source_home() {
        let entries = [entry(
            "Dialog",
            DocsStatus::ComponentCatalog,
            Some("Dialog"),
            false,
            "dialog.rs",
        )];

        let failures = audit_registry_entries(
            &entries,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &docs("", ""),
            |_| false,
            |_| false,
        );

        assert!(has_failure(&failures, "source_home `dialog.rs`"));
        assert!(has_failure(&failures, "Dialog"));
    }

    #[test]
    fn audit_reports_removed_primitive_reappearance() {
        let entries = [entry(
            "primitives::overlay",
            DocsStatus::Verification,
            Some("primitives::overlay"),
            false,
            "removed",
        )];

        let failures = audit_registry_entries(
            &entries,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &docs("", "primitive_deletion_target_inventory"),
            |_| true,
            |name| name == "primitives::overlay",
        );

        assert!(has_failure(
            &failures,
            "removed primitive `primitives::overlay`"
        ));
    }

    #[test]
    fn reexport_parser_expands_grouped_exports_and_aliases() {
        let source = r#"
pub use crate::button::{Button, ButtonState};
pub use crate::field::Field as FormField;
"#;

        let exports = reexport_tokens_from_source(source, Path::new("."), &mut Vec::new());

        assert!(exports.contains("Button"));
        assert!(exports.contains("ButtonState"));
        assert!(exports.contains("FormField"));
    }
}
