use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use syn::{Expr, Item, visit_mut::VisitMut};

use crate::theme_schema::theme_schema_failures;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContractRow {
    name: String,
    owner: String,
    gallery_status: String,
    docs_status: DocsStatus,
    docs_token: Option<String>,
    default_export: bool,
    source_inputs: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConformanceGate {
    id: String,
    evidence: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExecutableTestEvidence {
    source_path: &'static str,
    package: &'static str,
    test_target: &'static str,
    test_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactTestCommand {
    program: &'static str,
    args: Vec<String>,
}

const OLD_A11Y_EVIDENCE_IDENTIFIERS: &[&str] =
    &["ComponentA11yEvidence", "COMPONENT_A11Y_EVIDENCE"];
const PUBLIC_CLICK_EVENT_API_ALLOWLIST: &[(&str, &str)] = &[];

/// U5 runner coordinates for final-tree/action checkpoints.
///
/// U10 replaces these temporary coordinates with native scenario artifacts; component metadata
/// and Gallery display evidence must not become owners of test execution.
const U5_EXECUTABLE_A11Y_EVIDENCE: &[ExecutableTestEvidence] = &[
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/a11y/collection_semantics.rs",
        package: "open-gpui-ui-components",
        test_target: "a11y",
        test_name: "collection_semantics::listbox_final_tree_and_click_action_follow_resolved_state",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/a11y/collection_semantics.rs",
        package: "open-gpui-ui-components",
        test_target: "a11y",
        test_name: "collection_semantics::tree_final_tree_focus_click_and_expansion_follow_resolved_state",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/a11y/collection_semantics.rs",
        package: "open-gpui-ui-components",
        test_target: "a11y",
        test_name: "collection_semantics::virtualized_list_final_tree_distinguishes_rows_from_structural_content_and_recycles_by_key",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/a11y/collection_semantics.rs",
        package: "open-gpui-ui-components",
        test_target: "a11y",
        test_name: "collection_semantics::splitter_final_tree_actions_resize_and_disabled_state_remove_capability",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/a11y.rs",
        package: "open-gpui-ui-components",
        test_target: "a11y",
        test_name: "button_final_tree_and_actions_follow_resolved_projection",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/a11y_controls/action_controls.rs",
        package: "open-gpui-ui-components",
        test_target: "a11y_controls",
        test_name: "action_controls::checkbox_final_tree_tracks_form_state_actions_and_stable_identity",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/a11y_controls/field_relations.rs",
        package: "open-gpui-ui-components",
        test_target: "a11y_controls",
        test_name: "field_relations::field_relations_follow_help_error_transitions_and_unmount",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/choice.rs",
        package: "open-gpui-ui-components",
        test_target: "choice",
        test_name: "select_final_tree_preserves_trigger_identity_disabled_state_and_exact_actions",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/overlay.rs",
        package: "open-gpui-ui-components",
        test_target: "overlay",
        test_name: "dialog_final_tree_projects_modal_disabled_and_exact_actions",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/navigation.rs",
        package: "open-gpui-ui-components",
        test_target: "navigation",
        test_name: "tabs_final_tree_relations_actions_and_node_ids_follow_runtime_selection",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/primitives.rs",
        package: "open-gpui-ui-components",
        test_target: "primitives",
        test_name: "separator_final_tree_downgrades_to_group_preserves_orientation_and_omits_decorative_semantics",
    },
    ExecutableTestEvidence {
        source_path: "crates/ui_components/tests/table/accessibility.rs",
        package: "open-gpui-ui-components",
        test_target: "table",
        test_name: "accessibility::table_runtime_final_accessibility_tree",
    },
    ExecutableTestEvidence {
        source_path: "examples/ui-foundation-gallery/tests/foundation_gallery/focus_a11y_smoke.rs",
        package: "open-gpui-ui-foundation-gallery",
        test_target: "foundation_gallery",
        test_name: "focus_a11y_smoke::focus_a11y_devtools_allowlist_matches_final_tree_structure",
    },
];

pub(crate) fn scan_ui_contract(root: &Path) -> Result<(), ()> {
    println!("==> scan UI contract");

    let failures = ui_contract_failures(root);
    if !failures.is_empty() {
        eprintln!("UI contract scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        return Err(());
    }

    verify_executable_evidence(root)?;
    println!("UI contract scan passed");
    Ok(())
}

fn verify_executable_evidence(root: &Path) -> Result<(), ()> {
    println!("==> run executable UI conformance evidence");
    let failures =
        executable_evidence_failures(root, U5_EXECUTABLE_A11Y_EVIDENCE, |root, command| {
            let args = command.args.iter().map(String::as_str).collect::<Vec<_>>();
            crate::commands::run(root, command.program, &args)
        });

    if failures.is_empty() {
        Ok(())
    } else {
        eprintln!("Executable UI conformance evidence failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn executable_evidence_failures(
    root: &Path,
    evidence: &[ExecutableTestEvidence],
    mut execute: impl FnMut(&Path, &ExactTestCommand) -> Result<(), ()>,
) -> Vec<String> {
    let mut failures = Vec::new();

    for evidence in evidence {
        if let Err(failure) = executable_test_source(root, *evidence, |path| {
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
        }) {
            failures.push(failure);
            continue;
        }

        let command = exact_test_command(*evidence);
        if execute(root, &command).is_err() {
            failures.push(format!(
                "{}: exact non-ignored test `{}::{}` did not run successfully",
                evidence.source_path, evidence.test_target, evidence.test_name
            ));
        }
    }

    failures
}

fn exact_test_command(evidence: ExecutableTestEvidence) -> ExactTestCommand {
    ExactTestCommand {
        program: "cargo",
        args: [
            "nextest",
            "run",
            "--locked",
            "-p",
            evidence.package,
            "--test",
            evidence.test_target,
            "--ignore-default-filter",
            "--run-ignored",
            "default",
            "--no-tests",
            "fail",
            "-E",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain(std::iter::once(format!("test(={})", evidence.test_name)))
        .collect(),
    }
}

fn executable_test_source(
    root: &Path,
    evidence: ExecutableTestEvidence,
    mut read_source: impl FnMut(&Path) -> Result<String, String>,
) -> Result<PathBuf, String> {
    let target_root = integration_test_target_root(root, evidence)?;
    let target = syn::parse_file(&read_source(&target_root)?).map_err(|error| {
        format!(
            "{}: failed to parse test target `{}`: {error}",
            evidence.source_path, evidence.test_target
        )
    })?;
    let segments = evidence.test_name.split("::").collect::<Vec<_>>();
    let (actual_path, test_items, function_name) = match segments.as_slice() {
        [function_name] => (target_root.clone(), target.items, *function_name),
        [module_name, function_name] => {
            let modules = target
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Mod(item_mod) if item_mod.ident == *module_name => Some(item_mod),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [item_mod] = modules.as_slice() else {
                return Err(format!(
                    "{}: test target `{}` does not declare exactly one module `{module_name}`",
                    evidence.source_path, evidence.test_target
                ));
            };
            if let Some((_, items)) = &item_mod.content {
                (target_root.clone(), items.clone(), *function_name)
            } else {
                let path = test_module_path(&target_root, item_mod)?;
                let file = syn::parse_file(&read_source(&path)?).map_err(|error| {
                    format!(
                        "{}: failed to parse evidence source: {error}",
                        path.display()
                    )
                })?;
                (path, file.items, *function_name)
            }
        }
        _ => {
            return Err(format!(
                "{}: U5 executable coordinate must name a root test or one test module",
                evidence.source_path
            ));
        }
    };
    let declared_path = lexical_path(&root.join(evidence.source_path));
    let actual_path = lexical_path(&actual_path);
    if actual_path != declared_path {
        return Err(format!(
            "{}: stale executable evidence source coordinate; `{}` is owned by {}",
            evidence.source_path,
            evidence.test_name,
            repo_relative_path(root, &actual_path)
        ));
    }
    let matching_tests = test_items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(item_fn)
                if item_fn.sig.ident == function_name
                    && item_fn.attrs.iter().any(|attribute| {
                        attribute
                            .path()
                            .segments
                            .last()
                            .is_some_and(|segment| segment.ident == "test")
                    }) =>
            {
                Some(item_fn)
            }
            _ => None,
        })
        .count();
    if matching_tests != 1 {
        return Err(format!(
            "{}: declared source does not own exactly one test function `{}`",
            evidence.source_path, evidence.test_name
        ));
    }
    Ok(actual_path)
}

fn integration_test_target_root(
    root: &Path,
    evidence: ExecutableTestEvidence,
) -> Result<PathBuf, String> {
    let source_path = Path::new(evidence.source_path);
    let components = source_path.components().collect::<Vec<_>>();
    let Some(tests_index) = components
        .iter()
        .rposition(|component| component.as_os_str() == "tests")
    else {
        return Err(format!(
            "{}: executable evidence source must be under an integration `tests` directory",
            evidence.source_path
        ));
    };
    let mut tests_dir = PathBuf::new();
    for component in &components[..=tests_index] {
        tests_dir.push(component.as_os_str());
    }
    Ok(root
        .join(tests_dir)
        .join(format!("{}.rs", evidence.test_target)))
}

fn test_module_path(target_root: &Path, item_mod: &syn::ItemMod) -> Result<PathBuf, String> {
    let path = item_mod
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("path"))
        .and_then(|attribute| match &attribute.meta {
            syn::Meta::NameValue(value) => match &value.value {
                Expr::Lit(value) => match &value.lit {
                    syn::Lit::Str(path) => Some(PathBuf::from(path.value())),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .unwrap_or_else(|| {
            PathBuf::from(target_root.file_stem().unwrap_or_default())
                .join(format!("{}.rs", item_mod.ident))
        });
    Ok(lexical_path(
        &target_root
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(path),
    ))
}

fn lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn ui_contract_failures(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    let source_dir = root.join("crates/ui_components/src");
    let evidence_path = source_dir.join("component_contract/evidence.rs");
    let component_contract_docs_path = root.join("docs/ui/component-contract.md");
    let verification_docs_path = root.join("docs/verification.md");

    let row_sources = contract_row_sources(&source_dir, &mut failures);
    let evidence_source = read_to_string(&evidence_path, &mut failures);
    let component_contract_docs = read_to_string(&component_contract_docs_path, &mut failures);
    let verification_docs = read_to_string(&verification_docs_path, &mut failures);
    let root_exports = default_reexport_tokens(&source_dir, "lib.rs", &mut failures);
    let prelude_exports = default_reexport_tokens(&source_dir, "prelude.rs", &mut failures);
    let common_exports =
        default_reexport_tokens(&source_dir, "public_api/common.rs", &mut failures);

    if row_sources.is_empty() {
        return failures;
    }
    let Some(evidence_source) = evidence_source else {
        return failures;
    };
    let Some(component_contract_docs) = component_contract_docs else {
        return failures;
    };
    let Some(verification_docs) = verification_docs else {
        return failures;
    };

    let mut entries = Vec::new();
    for (source_path, source) in &row_sources {
        let (mut source_entries, parse_failures) = contract_rows_from_source(source);
        failures.extend(
            parse_failures
                .into_iter()
                .map(|failure| format!("{source_path}: {failure}")),
        );
        entries.append(&mut source_entries);
    }

    let docs = Docs {
        component_contract: &component_contract_docs,
        verification: &verification_docs,
    };
    failures.extend(audit_contract_rows(
        &entries,
        &root_exports,
        &prelude_exports,
        &common_exports,
        &docs,
        |entry| source_home_exists(&source_dir, entry),
        |name| removed_primitive_module_exists(&source_dir, name),
    ));
    failures.extend(audit_conformance_gates(&evidence_source));
    failures.extend(audit_semantic_authority(root, &source_dir, &entries));
    failures.extend(theme_schema_failures(root));

    failures
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
                "failed to read crates/ui_components/src/component_contract/rows: {error}"
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

fn read_to_string(path: &Path, failures: &mut Vec<String>) -> Option<String> {
    fs::read_to_string(path).map_or_else(
        |error| {
            failures.push(format!("{}: failed to read file: {error}", path.display()));
            None
        },
        Some,
    )
}

fn audit_semantic_authority(
    root: &Path,
    source_dir: &Path,
    contract_rows: &[ContractRow],
) -> Vec<String> {
    let mut failures = Vec::new();
    let source_inputs = contract_rows
        .iter()
        .filter(|row| {
            matches!(
                row.owner.as_str(),
                "OfficialComponent" | "OfficialComponentRecipe"
            )
        })
        .flat_map(|row| row.source_inputs.iter().cloned())
        .collect::<BTreeSet<_>>();

    let producer_sources =
        expand_official_producer_sources(source_dir, &source_inputs, &mut failures);
    for path in producer_sources {
        let label = repo_relative_path(root, &path);
        if let Some(source) = read_to_string(&path, &mut failures) {
            failures.extend(direct_semantic_assembly_failures(&label, &source));
        }
    }

    let mut runtime_sources = BTreeSet::new();
    for runtime_root in [
        root.join("crates/ui_components/src"),
        root.join("crates/devtools/src"),
        root.join("examples/ui-foundation-gallery/src"),
    ] {
        collect_rust_sources(&runtime_root, &mut runtime_sources, &mut failures);
    }
    for path in runtime_sources {
        let label = repo_relative_path(root, &path);
        if let Some(source) = read_to_string(&path, &mut failures) {
            if path.starts_with(source_dir) {
                failures.extend(public_click_event_api_failures(&label, &source));
            }
            failures.extend(old_a11y_evidence_failures(&label, &source));
        }
    }

    failures
}

fn unwrapped_expr(mut expression: &Expr) -> &Expr {
    loop {
        expression = match expression {
            Expr::Reference(reference) => &reference.expr,
            Expr::Group(group) => &group.expr,
            Expr::Paren(paren) => &paren.expr,
            _ => return expression,
        };
    }
}

fn expand_official_producer_sources(
    source_dir: &Path,
    source_inputs: &BTreeSet<String>,
    failures: &mut Vec<String>,
) -> BTreeSet<PathBuf> {
    let mut sources = BTreeSet::new();
    for input in source_inputs {
        let path = source_dir.join(input);
        if path.is_dir() {
            collect_rust_sources(&path, &mut sources, failures);
        } else if path.is_file() {
            if path.file_name().is_some_and(|name| name == "mod.rs") {
                collect_rust_sources(path.parent().unwrap_or(source_dir), &mut sources, failures);
            } else {
                sources.insert(path);
            }
        } else {
            failures.push(format!(
                "crates/ui_components/src/component_contract/rows/catalog.rs: official producer source input `{input}` does not exist"
            ));
        }
    }
    sources
}

fn collect_rust_sources(
    directory: &Path,
    sources: &mut BTreeSet<PathBuf>,
    failures: &mut Vec<String>,
) {
    let mut entries = match fs::read_dir(directory) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(error) => {
            failures.push(format!(
                "{}: failed to read Rust source directory: {error}",
                directory.display()
            ));
            return;
        }
    };
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, sources, failures);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.insert(path);
        }
    }
}

fn direct_semantic_assembly_failures(source_path: &str, source: &str) -> Vec<String> {
    let mut file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => {
            return vec![format!(
                "{source_path}: failed to parse official producer source: {error}"
            )];
        }
    };
    let mut visitor = DirectSemanticAssemblyVisitor {
        source_path,
        failures: Vec::new(),
    };
    visitor.visit_file_mut(&mut file);
    visitor.failures
}

struct DirectSemanticAssemblyVisitor<'a> {
    source_path: &'a str,
    failures: Vec<String>,
}

impl VisitMut for DirectSemanticAssemblyVisitor<'_> {
    fn visit_expr_method_call_mut(&mut self, call: &mut syn::ExprMethodCall) {
        let method = call.method.to_string();
        let forbidden =
            method == "ui_role" || method.starts_with("ui_aria_") || method.starts_with("aria_");
        if forbidden && !semantic_method_allowlisted(self.source_path, &method) {
            self.failures.push(format!(
                "{}: official producer directly calls semantic assembly method `.{method}(...)`; project a SemanticDescriptor through the shared adapter",
                self.source_path
            ));
        }
        syn::visit_mut::visit_expr_method_call_mut(self, call);
    }
}

fn semantic_method_allowlisted(source_path: &str, method: &str) -> bool {
    matches!(
        (source_path, method),
        (
            "crates/ui_components/src/table/behavior/columns.rs",
            "aria_column_count" | "aria_column_index"
        ) | (
            "crates/ui_components/src/table/behavior/counts.rs",
            "aria_row_count"
        ) | (
            "crates/ui_components/src/table/behavior/mod.rs",
            "aria_rows" | "aria_columns"
        ) | (
            "crates/ui_components/src/table/behavior/rows.rs",
            "aria_row_index" | "aria_column_index"
        ) | (
            "crates/ui_components/src/table/body/rows.rs",
            "aria_row_index"
        ) | (
            "crates/ui_components/src/table/cell.rs",
            "aria_column_index"
        ) | (
            "crates/ui_components/src/table/header.rs",
            "aria_column_index"
        ) | (
            "crates/ui_components/src/table/mod.rs",
            "aria_row_count" | "aria_column_count"
        ) | (
            "crates/ui_components/src/table/render_plan/rows.rs",
            "aria_column_index"
        ) | ("crates/ui_components/src/table/editors.rs", "aria_label")
    )
}

fn public_click_event_api_failures(source_path: &str, source: &str) -> Vec<String> {
    let mut file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => {
            return vec![format!(
                "{source_path}: failed to parse UI Components public API: {error}"
            )];
        }
    };
    let forbidden_types = click_event_type_identifiers(&file);
    let mut visitor = PublicClickEventApiVisitor {
        source_path,
        forbidden_types: &forbidden_types,
        impl_owner: None,
        public_trait_owner: None,
        failures: Vec::new(),
    };
    visitor.visit_file_mut(&mut file);
    visitor.failures
}

struct PublicClickEventApiVisitor<'a> {
    source_path: &'a str,
    forbidden_types: &'a BTreeSet<String>,
    impl_owner: Option<String>,
    public_trait_owner: Option<String>,
    failures: Vec<String>,
}

impl PublicClickEventApiVisitor<'_> {
    fn reject(&mut self, api: String) {
        if PUBLIC_CLICK_EVENT_API_ALLOWLIST
            .iter()
            .any(|&(path, allowed_api)| path == self.source_path && allowed_api == api)
        {
            return;
        }

        self.failures.push(format!(
            "{}: public API `{api}` exposes `ClickEvent`; use typed semantic activation or value intent instead of a physical-click callback",
            self.source_path
        ));
    }
}

impl VisitMut for PublicClickEventApiVisitor<'_> {
    fn visit_item_fn_mut(&mut self, item: &mut syn::ItemFn) {
        if visibility_is_public(&item.vis)
            && signature_mentions_click_event(&item.sig, self.forbidden_types)
        {
            self.reject(item.sig.ident.to_string());
        }
    }

    fn visit_item_impl_mut(&mut self, item: &mut syn::ItemImpl) {
        let owner = type_owner_name(&item.self_ty);
        let previous = self.impl_owner.replace(owner);
        syn::visit_mut::visit_item_impl_mut(self, item);
        self.impl_owner = previous;
    }

    fn visit_impl_item_fn_mut(&mut self, item: &mut syn::ImplItemFn) {
        if visibility_is_public(&item.vis)
            && signature_mentions_click_event(&item.sig, self.forbidden_types)
        {
            let owner = self.impl_owner.as_deref().unwrap_or("<impl>");
            self.reject(format!("{owner}::{}", item.sig.ident));
        }
    }

    fn visit_item_trait_mut(&mut self, item: &mut syn::ItemTrait) {
        let is_public = visibility_is_public(&item.vis);
        if is_public
            && (generics_mention_click_event(&item.generics, self.forbidden_types)
                || bounds_mention_click_event(&item.supertraits, self.forbidden_types))
        {
            self.reject(item.ident.to_string());
        }
        let owner = is_public.then(|| item.ident.to_string());
        let previous = std::mem::replace(&mut self.public_trait_owner, owner);
        syn::visit_mut::visit_item_trait_mut(self, item);
        self.public_trait_owner = previous;
    }

    fn visit_trait_item_fn_mut(&mut self, item: &mut syn::TraitItemFn) {
        if signature_mentions_click_event(&item.sig, self.forbidden_types) {
            if let Some(owner) = self.public_trait_owner.clone() {
                self.reject(format!("{owner}::{}", item.sig.ident));
            }
        }
    }

    fn visit_trait_item_type_mut(&mut self, item: &mut syn::TraitItemType) {
        let exposes_click_event =
            generics_mention_click_event(&item.generics, self.forbidden_types)
                || bounds_mention_click_event(&item.bounds, self.forbidden_types)
                || item
                    .default
                    .as_ref()
                    .is_some_and(|(_, ty)| type_mentions_click_event(ty, self.forbidden_types));
        if exposes_click_event {
            if let Some(owner) = self.public_trait_owner.clone() {
                self.reject(format!("{owner}::{}", item.ident));
            }
        }
    }

    fn visit_trait_item_const_mut(&mut self, item: &mut syn::TraitItemConst) {
        if type_mentions_click_event(&item.ty, self.forbidden_types) {
            if let Some(owner) = self.public_trait_owner.clone() {
                self.reject(format!("{owner}::{}", item.ident));
            }
        }
    }

    fn visit_item_type_mut(&mut self, item: &mut syn::ItemType) {
        if visibility_is_public(&item.vis)
            && (generics_mention_click_event(&item.generics, self.forbidden_types)
                || type_mentions_click_event(&item.ty, self.forbidden_types))
        {
            self.reject(item.ident.to_string());
        }
    }

    fn visit_item_struct_mut(&mut self, item: &mut syn::ItemStruct) {
        if visibility_is_public(&item.vis) {
            if generics_mention_click_event(&item.generics, self.forbidden_types) {
                self.reject(item.ident.to_string());
            }
            for (index, field) in item.fields.iter().enumerate() {
                if visibility_is_public(&field.vis)
                    && type_mentions_click_event(&field.ty, self.forbidden_types)
                {
                    let field = field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| index.to_string());
                    self.reject(format!("{}::{field}", item.ident));
                }
            }
        }
    }

    fn visit_item_enum_mut(&mut self, item: &mut syn::ItemEnum) {
        if visibility_is_public(&item.vis) {
            if generics_mention_click_event(&item.generics, self.forbidden_types) {
                self.reject(item.ident.to_string());
            }
            for variant in &item.variants {
                if variant
                    .fields
                    .iter()
                    .any(|field| type_mentions_click_event(&field.ty, self.forbidden_types))
                {
                    self.reject(format!("{}::{}", item.ident, variant.ident));
                }
            }
        }
    }

    fn visit_item_union_mut(&mut self, item: &mut syn::ItemUnion) {
        if visibility_is_public(&item.vis) {
            if generics_mention_click_event(&item.generics, self.forbidden_types) {
                self.reject(item.ident.to_string());
            }
            for field in &item.fields.named {
                if visibility_is_public(&field.vis)
                    && type_mentions_click_event(&field.ty, self.forbidden_types)
                {
                    let field = field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "<field>".to_owned());
                    self.reject(format!("{}::{field}", item.ident));
                }
            }
        }
    }

    fn visit_item_const_mut(&mut self, item: &mut syn::ItemConst) {
        if visibility_is_public(&item.vis)
            && type_mentions_click_event(&item.ty, self.forbidden_types)
        {
            self.reject(item.ident.to_string());
        }
    }

    fn visit_item_static_mut(&mut self, item: &mut syn::ItemStatic) {
        if visibility_is_public(&item.vis)
            && type_mentions_click_event(&item.ty, self.forbidden_types)
        {
            self.reject(item.ident.to_string());
        }
    }
}

fn visibility_is_public(visibility: &syn::Visibility) -> bool {
    matches!(visibility, syn::Visibility::Public(_))
}

fn signature_mentions_click_event(
    signature: &syn::Signature,
    forbidden_types: &BTreeSet<String>,
) -> bool {
    let mut signature = signature.clone();
    let mut visitor = ClickEventTypeVisitor::new(forbidden_types);
    visitor.visit_signature_mut(&mut signature);
    visitor.found
}

fn type_mentions_click_event(ty: &syn::Type, forbidden_types: &BTreeSet<String>) -> bool {
    let mut ty = ty.clone();
    let mut visitor = ClickEventTypeVisitor::new(forbidden_types);
    visitor.visit_type_mut(&mut ty);
    visitor.found
}

fn generics_mention_click_event(
    generics: &syn::Generics,
    forbidden_types: &BTreeSet<String>,
) -> bool {
    let mut generics = generics.clone();
    let mut visitor = ClickEventTypeVisitor::new(forbidden_types);
    visitor.visit_generics_mut(&mut generics);
    visitor.found
}

fn bounds_mention_click_event(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Plus>,
    forbidden_types: &BTreeSet<String>,
) -> bool {
    bounds.iter().any(|bound| {
        let mut bound = bound.clone();
        let mut visitor = ClickEventTypeVisitor::new(forbidden_types);
        visitor.visit_type_param_bound_mut(&mut bound);
        visitor.found
    })
}

fn type_owner_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_else(|| "<impl>".to_owned()),
        _ => "<impl>".to_owned(),
    }
}

fn click_event_type_identifiers(file: &syn::File) -> BTreeSet<String> {
    let mut identifiers = BTreeSet::from(["ClickEvent".to_owned()]);
    collect_click_event_import_aliases(&file.items, &mut identifiers);

    loop {
        let previous_len = identifiers.len();
        collect_click_event_type_aliases(&file.items, &mut identifiers);
        if identifiers.len() == previous_len {
            return identifiers;
        }
    }
}

fn collect_click_event_import_aliases(items: &[Item], identifiers: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Use(item) => collect_click_event_use_aliases(&item.tree, false, identifiers),
            Item::Mod(item) => {
                if let Some((_, items)) = &item.content {
                    collect_click_event_import_aliases(items, identifiers);
                }
            }
            _ => {}
        }
    }
}

fn collect_click_event_use_aliases(
    tree: &syn::UseTree,
    click_event_path: bool,
    identifiers: &mut BTreeSet<String>,
) {
    match tree {
        syn::UseTree::Path(path) => collect_click_event_use_aliases(
            &path.tree,
            click_event_path || path.ident == "ClickEvent",
            identifiers,
        ),
        syn::UseTree::Name(name) => {
            if click_event_path || name.ident == "ClickEvent" {
                identifiers.insert(name.ident.to_string());
            }
        }
        syn::UseTree::Rename(rename) => {
            if click_event_path || rename.ident == "ClickEvent" {
                identifiers.insert(rename.rename.to_string());
            }
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect_click_event_use_aliases(tree, click_event_path, identifiers);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn collect_click_event_type_aliases(items: &[Item], identifiers: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Type(item)
                if generics_mention_click_event(&item.generics, identifiers)
                    || type_mentions_click_event(&item.ty, identifiers) =>
            {
                identifiers.insert(item.ident.to_string());
            }
            Item::Mod(item) => {
                if let Some((_, items)) = &item.content {
                    collect_click_event_type_aliases(items, identifiers);
                }
            }
            _ => {}
        }
    }
}

struct ClickEventTypeVisitor<'a> {
    forbidden_types: &'a BTreeSet<String>,
    found: bool,
}

impl<'a> ClickEventTypeVisitor<'a> {
    fn new(forbidden_types: &'a BTreeSet<String>) -> Self {
        Self {
            forbidden_types,
            found: false,
        }
    }
}

impl VisitMut for ClickEventTypeVisitor<'_> {
    fn visit_path_segment_mut(&mut self, segment: &mut syn::PathSegment) {
        if self
            .forbidden_types
            .iter()
            .any(|identifier| segment.ident == identifier.as_str())
        {
            self.found = true;
        }
        syn::visit_mut::visit_path_segment_mut(self, segment);
    }
}

fn old_a11y_evidence_failures(source_path: &str, source: &str) -> Vec<String> {
    let mut file = match syn::parse_file(source) {
        Ok(file) => file,
        Err(error) => {
            return vec![format!(
                "{source_path}: failed to parse runtime source for old a11y evidence audit: {error}"
            )];
        }
    };
    let mut visitor = OldA11yEvidenceVisitor::default();
    for item in &mut file.items {
        if !old_a11y_scaffold_item_allowed(source_path, item) {
            visitor.visit_item_mut(item);
        }
    }
    if visitor.identifiers.is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "{source_path}: runtime source consumes U10-only a11y evidence scaffold {}",
            visitor
                .identifiers
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        )]
    }
}

#[derive(Default)]
struct OldA11yEvidenceVisitor {
    identifiers: BTreeSet<String>,
}

impl VisitMut for OldA11yEvidenceVisitor {
    fn visit_ident_mut(&mut self, identifier: &mut syn::Ident) {
        let identifier = identifier.to_string();
        if OLD_A11Y_EVIDENCE_IDENTIFIERS.contains(&identifier.as_str()) {
            self.identifiers.insert(identifier);
        }
    }
}

fn old_a11y_scaffold_item_allowed(source_path: &str, item: &Item) -> bool {
    match (source_path, item) {
        ("crates/ui_components/src/component_contract/types.rs", Item::Struct(item)) => {
            item.ident == "ComponentA11yEvidence"
        }
        ("crates/ui_components/src/component_contract/evidence.rs", Item::Const(item)) => {
            item.ident == "COMPONENT_A11Y_EVIDENCE"
                && matches!(unwrapped_expr(&item.expr), Expr::Array(array) if array.elems.is_empty())
        }
        (
            "crates/ui_components/src/component_contract/evidence.rs"
            | "crates/ui_components/src/component_contract/mod.rs"
            | "crates/ui_components/src/public_api/common.rs"
            | "crates/ui_components/src/public_api/default.rs",
            Item::Use(_),
        ) => true,
        _ => false,
    }
}

fn audit_contract_rows(
    entries: &[ContractRow],
    root_exports: &BTreeSet<String>,
    prelude_exports: &BTreeSet<String>,
    common_exports: &BTreeSet<String>,
    docs: &Docs<'_>,
    mut source_home_exists: impl FnMut(&ContractRow) -> bool,
    mut removed_primitive_exists: impl FnMut(&str) -> bool,
) -> Vec<String> {
    let mut failures = Vec::new();
    let mut owners = BTreeMap::new();

    for entry in entries {
        if let Some(previous) = owners.insert(entry.name.as_str(), entry.source_home.as_str()) {
            failures.push(format!(
                "crates/ui_components/src/component_contract/rows.rs: contract row `{}` is duplicated; previous source_home `{previous}`, duplicate source_home `{}`",
                entry.name, entry.source_home
            ));
        }

        audit_gallery_status(entry, &mut failures);

        if entry.default_export {
            if !root_exports.contains(&entry.name) {
                failures.push(format!(
                    "crates/ui_components/src/lib.rs: default-export contract row `{}` is missing from crate root exports; add it to crates/ui_components/src/public_api/default.rs or explicitly re-export it",
                    entry.name
                ));
            }
            if !prelude_exports.contains(&entry.name) {
                if common_exports.contains(&entry.name) {
                    failures.push(format!(
                        "crates/ui_components/src/prelude.rs: common contract row `{}` is missing from prelude exports; add it to crates/ui_components/src/public_api/common.rs or explicitly re-export it",
                        entry.name
                    ));
                }
            }
        }

        if entry.source_home == "removed" {
            if removed_primitive_exists(&entry.name) {
                failures.push(format!(
                    "crates/ui_components/src/primitives/mod.rs: removed primitive `{}` reappeared; delete the compatibility module or update the contract ownership",
                    entry.name
                ));
            }
        } else if !source_home_exists(entry) {
            failures.push(format!(
                "crates/ui_components/src/component_contract/rows.rs: contract row `{}` source_home `{}` does not exist under crates/ui_components/src",
                entry.name, entry.source_home
            ));
        }

        if let Some(token) = &entry.docs_token {
            audit_docs_token(entry, token, docs, &mut failures);
        }
    }

    failures
}

fn audit_docs_token(entry: &ContractRow, token: &str, docs: &Docs<'_>, failures: &mut Vec<String>) {
    match entry.docs_status {
        DocsStatus::ComponentCatalog => {}
        DocsStatus::ComponentContract => {
            if !docs.component_contract.contains(token) {
                failures.push(format!(
                    "docs/ui/component-contract.md: missing docs token `{token}` for contract row `{}`",
                    entry.name
                ));
            }
        }
        DocsStatus::ComponentContractOrVerification => {
            if !docs.component_contract.contains(token) && !docs.verification.contains(token) {
                failures.push(format!(
                    "docs/ui/component-contract.md or docs/verification.md: missing docs token `{token}` for contract row `{}`",
                    entry.name
                ));
            }
        }
        DocsStatus::Verification => {
            if !docs.verification.contains(token)
                && entry.source_home == "removed"
                && !docs
                    .verification
                    .contains("primitive_deletion_target_inventory")
            {
                failures.push(format!(
                    "docs/verification.md: missing docs token `{token}` for contract row `{}`",
                    entry.name
                ));
            } else if !docs.verification.contains(token) && entry.source_home != "removed" {
                failures.push(format!(
                    "docs/verification.md: missing docs token `{token}` for contract row `{}`",
                    entry.name
                ));
            }
        }
    }
}

fn audit_gallery_status(entry: &ContractRow, failures: &mut Vec<String>) {
    let valid_status = match entry.owner.as_str() {
        "OfficialComponent" => matches!(
            entry.gallery_status.as_str(),
            "OfficialComponent" | "OfficialOverlay"
        ),
        "OfficialComponentRecipe" => entry.gallery_status == "NotInGallery",
        "RendererNeutralStateContract" => {
            matches!(
                entry.gallery_status.as_str(),
                "StateContract" | "NotInGallery"
            )
        }
        "GpuiAdapterHelper" => {
            matches!(
                entry.gallery_status.as_str(),
                "AdapterOnly" | "NotInGallery"
            )
        }
        "InternalImplementationDetail" => {
            matches!(
                entry.gallery_status.as_str(),
                "InternalAnatomy" | "NotInGallery"
            )
        }
        "DeprecatedRemovalTarget" | "DiagnosticSurface" => entry.gallery_status == "NotInGallery",
        _ => false,
    };

    if !valid_status {
        failures.push(format!(
            "crates/ui_components/src/component_contract/rows.rs: contract row `{}` owner `{}` is incompatible with gallery_status `{}`; align the row with SurfaceGalleryStatus ownership rules",
            entry.name, entry.owner, entry.gallery_status
        ));
    }
}

fn source_home_exists(source_dir: &Path, entry: &ContractRow) -> bool {
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

fn contract_rows_from_source(source: &str) -> (Vec<ContractRow>, Vec<String>) {
    let (blocks, block_failures) = struct_literal_blocks(source, "ComponentContractEntry");
    let mut entries = Vec::new();
    let mut failures = Vec::new();
    failures.extend(block_failures);

    for block in blocks {
        match contract_row_from_block(block) {
            Ok(entry) => entries.push(entry),
            Err(error) => failures.push(error),
        }
    }

    (entries, failures)
}

fn contract_row_from_block(block: &str) -> Result<ContractRow, String> {
    let name = string_field(block, "name").ok_or("contract row missing `name`")?;
    let owner = enum_variant_field(block, "owner")
        .ok_or_else(|| format!("contract row `{name}` missing or has unknown `owner`"))?;
    let gallery_status = enum_variant_field(block, "gallery_status")
        .ok_or_else(|| format!("contract row `{name}` missing or has unknown `gallery_status`"))?;
    let docs_status = docs_status_field(block)
        .ok_or_else(|| format!("contract row `{name}` missing or has unknown `docs_status`"))?;
    let docs_token = optional_string_field(block, "docs_token");
    let default_export = bool_field(block, "default_export")
        .ok_or_else(|| format!("contract row `{name}` missing `default_export`"))?;
    let source_inputs = bracketed_slice_field(block, "source_inputs")
        .map(quoted_strings)
        .ok_or_else(|| format!("contract row `{name}` missing `source_inputs`"))?;
    let source_home = string_field(block, "source_home")
        .ok_or_else(|| format!("contract row `{name}` missing `source_home`"))?;

    Ok(ContractRow {
        name,
        owner,
        gallery_status,
        docs_status,
        docs_token,
        default_export,
        source_inputs,
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
    enum_variant_from_source(rest)
}

fn field_value<'a>(block: &'a str, field: &str) -> Option<&'a str> {
    Some(field_tail(block, field)?.lines().next().unwrap_or_default())
}

fn field_tail<'a>(block: &'a str, field: &str) -> Option<&'a str> {
    let marker = format!("{field}:");
    let start = block.find(&marker)? + marker.len();
    Some(&block[start..])
}

fn audit_conformance_gates(evidence_source: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let (gates, gate_parse_failures) = conformance_gates_from_source(evidence_source);
    failures.extend(gate_parse_failures.into_iter().map(|failure| {
        format!("crates/ui_components/src/component_contract/evidence.rs: {failure}")
    }));
    failures.extend(audit_conformance_gate_evidence(&gates));

    failures
}

fn conformance_gates_from_source(source: &str) -> (Vec<ConformanceGate>, Vec<String>) {
    let (blocks, block_failures) = struct_literal_blocks(source, "ComponentConformanceGate");
    let mut gates = Vec::new();
    let mut failures = block_failures;

    for block in blocks {
        match conformance_gate_from_block(block) {
            Ok(gate) => gates.push(gate),
            Err(error) => failures.push(error),
        }
    }

    (gates, failures)
}

fn conformance_gate_from_block(block: &str) -> Result<ConformanceGate, String> {
    let id = string_field(block, "id").ok_or("conformance gate missing `id`")?;
    let evidence = evidence_field(block)
        .ok_or_else(|| format!("conformance gate `{id}` missing `evidence`"))?;
    Ok(ConformanceGate { id, evidence })
}

fn evidence_field(block: &str) -> Option<BTreeSet<String>> {
    Some(
        quoted_strings(bracketed_slice_field(block, "evidence")?)
            .into_iter()
            .collect(),
    )
}

fn bracketed_slice_field<'a>(block: &'a str, field: &str) -> Option<&'a str> {
    let rest = field_tail(block, field)?.trim_start();
    let open = rest.find("&[")? + 1;
    let close = matching_bracket(rest, open)?;
    Some(&rest[open + 1..close])
}

fn audit_conformance_gate_evidence(gates: &[ConformanceGate]) -> Vec<String> {
    let mut failures = Vec::new();
    let all_evidence = gates
        .iter()
        .flat_map(|gate| gate.evidence.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let gate_ids = gates
        .iter()
        .map(|gate| gate.id.as_str())
        .collect::<BTreeSet<_>>();

    for required_gate in [
        "public-api-exports",
        "gallery-metadata",
        "a11y-labels",
        "theme-schema",
    ] {
        if !gate_ids.contains(required_gate) {
            failures.push(format!(
                "crates/ui_components/src/component_contract/evidence.rs: COMPONENT_CONFORMANCE_GATES is missing `{required_gate}`"
            ));
        }
    }

    for (class, token) in [
        (
            "contract",
            "crates/ui_components/src/component_contract/rows.rs",
        ),
        (
            "contract",
            "crates/ui_components/src/component_contract/evidence.rs",
        ),
        ("contract", "crates/ui_components/tests/public_surface.rs"),
        (
            "gallery",
            "examples/ui-foundation-gallery/tests/foundation_gallery.rs",
        ),
        ("theme", "crates/ui_components/src/theme/schema.rs"),
        ("theme", "crates/ui_components/tests/theme.rs"),
        ("theme", "docs/schemas/open-gpui-theme-v1.schema.json"),
        ("theme", "cargo run -p xtask -- scan-theme-drift"),
        ("theme", "cargo run -p xtask -- scan-theme-schema"),
    ] {
        if !all_evidence.contains(token) {
            failures.push(format!(
                "crates/ui_components/src/component_contract/evidence.rs: conformance evidence is missing {class} owner `{token}`"
            ));
        }
    }

    failures
}

fn struct_literal_blocks<'a>(source: &'a str, type_name: &str) -> (Vec<&'a str>, Vec<String>) {
    let mut blocks = Vec::new();
    let mut failures = Vec::new();
    let mut search_from = 0;

    while let Some(relative_start) = source[search_from..].find(type_name) {
        let start = search_from + relative_start;
        let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
        let line_prefix = &source[line_start..start];
        let trimmed_line_prefix = line_prefix.trim_start();
        if line_prefix.contains("struct")
            || line_prefix.contains("fn ")
            || trimmed_line_prefix.starts_with("impl ")
            || trimmed_line_prefix.starts_with("use ")
            || trimmed_line_prefix.starts_with("pub use ")
            || trimmed_line_prefix.starts_with("///")
            || trimmed_line_prefix.starts_with("//!")
        {
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

fn enum_variant_from_source(source: &str) -> Option<String> {
    let variant_start = source.rfind("::").map(|index| index + 2).unwrap_or(0);
    let variant = source[variant_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    (!variant.is_empty()).then_some(variant)
}

fn quoted_strings(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find('"') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('"') else {
            break;
        };
        values.push(rest[..end].to_string());
        rest = &rest[end + 1..];
    }
    values
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

fn matching_bracket(source: &str, open_bracket: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (offset, ch) in source[open_bracket..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_bracket + offset);
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
    use crate::commands::workspace_root;
    use serde::Deserialize;

    #[derive(Clone, Debug, Deserialize)]
    struct NextestEvidenceFixture {
        tests: Vec<NextestFixtureTest>,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct NextestFixtureTest {
        package: String,
        test_target: String,
        test_name: String,
        ignored: bool,
    }

    fn entry(
        name: &str,
        owner: &str,
        gallery_status: &str,
        docs_status: DocsStatus,
        docs_token: Option<&str>,
        default_export: bool,
        source_home: &str,
    ) -> ContractRow {
        ContractRow {
            name: name.to_string(),
            owner: owner.to_string(),
            gallery_status: gallery_status.to_string(),
            docs_status,
            docs_token: docs_token.map(str::to_string),
            default_export,
            source_inputs: Vec::new(),
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

    fn nextest_evidence_fixture() -> NextestEvidenceFixture {
        serde_json::from_str(include_str!(
            "../tests/fixtures/ui-contract-nextest-evidence.json"
        ))
        .expect("nextest evidence fixture should be valid JSON")
    }

    fn fixture_executor(
        fixture: &NextestEvidenceFixture,
    ) -> impl FnMut(&Path, &ExactTestCommand) -> Result<(), ()> + '_ {
        move |_root, command| {
            let argument_after = |flag: &str| {
                command
                    .args
                    .iter()
                    .position(|argument| argument == flag)
                    .and_then(|index| command.args.get(index + 1))
                    .map(String::as_str)
            };
            let exact_name = argument_after("-E")
                .and_then(|filter| filter.strip_prefix("test(="))
                .and_then(|filter| filter.strip_suffix(')'));

            let command_is_strict = command.program == "cargo"
                && command
                    .args
                    .starts_with(&["nextest".to_string(), "run".to_string()])
                && command.args.iter().any(|argument| argument == "--locked")
                && argument_after("--no-tests") == Some("fail")
                && argument_after("--run-ignored") == Some("default")
                && !command.args.iter().any(|argument| argument == "--exact")
                && command
                    .args
                    .iter()
                    .any(|argument| argument == "--ignore-default-filter");
            let matched = fixture.tests.iter().any(|test| {
                argument_after("-p") == Some(test.package.as_str())
                    && argument_after("--test") == Some(test.test_target.as_str())
                    && exact_name == Some(test.test_name.as_str())
                    && !test.ignored
            });

            (command_is_strict && matched).then_some(()).ok_or(())
        }
    }

    fn fixture_evidence() -> ExecutableTestEvidence {
        U5_EXECUTABLE_A11Y_EVIDENCE[0]
    }

    fn assert_fixture_rejected(fixture: &NextestEvidenceFixture, needle: &str) {
        let failures = executable_evidence_failures(
            workspace_root(),
            &[fixture_evidence()],
            fixture_executor(fixture),
        );

        assert!(
            has_failure(&failures, needle),
            "expected `{needle}` in {failures:#?}"
        );
    }

    #[test]
    fn executable_evidence_runs_an_exact_non_ignored_test() {
        let fixture = nextest_evidence_fixture();

        let failures = executable_evidence_failures(
            workspace_root(),
            &[fixture_evidence()],
            fixture_executor(&fixture),
        );

        assert_eq!(failures, Vec::<String>::new());
    }

    #[test]
    fn executable_evidence_rejects_a_missing_source_path() {
        let fixture = nextest_evidence_fixture();
        let mut evidence = fixture_evidence();
        evidence.source_path = "crates/ui_components/tests/a11y/missing.rs";

        let failures =
            executable_evidence_failures(workspace_root(), &[evidence], fixture_executor(&fixture));

        assert!(has_failure(&failures, "missing.rs"));
    }

    #[test]
    fn executable_evidence_rejects_a_missing_test() {
        let mut fixture = nextest_evidence_fixture();
        fixture.tests.clear();

        assert_fixture_rejected(
            &fixture,
            "listbox_final_tree_and_click_action_follow_resolved_state",
        );
    }

    #[test]
    fn executable_evidence_rejects_a_test_moved_to_another_target() {
        let mut fixture = nextest_evidence_fixture();
        fixture.tests[0].test_target = "renamed_a11y".to_string();

        assert_fixture_rejected(&fixture, "a11y");
    }

    #[test]
    fn executable_evidence_rejects_a_renamed_test_symbol() {
        let mut fixture = nextest_evidence_fixture();
        fixture.tests[0].test_name = "collection_semantics::renamed_test".to_string();

        assert_fixture_rejected(
            &fixture,
            "listbox_final_tree_and_click_action_follow_resolved_state",
        );
    }

    #[test]
    fn executable_evidence_rejects_an_ignored_test() {
        let mut fixture = nextest_evidence_fixture();
        fixture.tests[0].ignored = true;

        assert_fixture_rejected(&fixture, "non-ignored");
    }

    #[test]
    fn executable_evidence_rejects_a_stale_source_coordinate_after_path_move() {
        let root = Path::new("fixture-workspace");
        let target = root.join("crates/ui_components/tests/a11y.rs");
        let stale = root.join("crates/ui_components/tests/a11y/collection_semantics.rs");
        let moved = root.join("crates/ui_components/tests/a11y/moved.rs");
        let sources = BTreeMap::from([
            (
                target,
                "#[path = \"a11y/moved.rs\"]\nmod collection_semantics;".to_string(),
            ),
            (
                stale,
                "#[test]\nfn listbox_final_tree_and_click_action_follow_resolved_state() {}"
                    .to_string(),
            ),
            (
                moved,
                "#[test]\nfn listbox_final_tree_and_click_action_follow_resolved_state() {}"
                    .to_string(),
            ),
        ]);

        let failure = executable_test_source(root, fixture_evidence(), |path| {
            sources
                .get(path)
                .cloned()
                .ok_or_else(|| "fixture source is missing".to_string())
        })
        .expect_err("moved test must invalidate the retained source coordinate");

        assert!(failure.contains("stale executable evidence source coordinate"));
        assert!(failure.contains("a11y/moved.rs"));
    }

    #[test]
    fn official_producer_audit_rejects_direct_semantic_assembly_methods() {
        for method_call in [
            "element.ui_role(Role::Button)",
            "element.ui_aria_disabled(true)",
            "element.aria_label(\"label\")",
        ] {
            let source = format!("fn render(element: Element) {{ let _ = {method_call}; }}");
            let failures =
                direct_semantic_assembly_failures("crates/ui_components/src/button.rs", &source);

            assert!(
                has_failure(&failures, "directly calls semantic assembly method"),
                "expected `{method_call}` to fail: {failures:#?}"
            );
        }
    }

    #[test]
    fn official_producer_audit_keeps_narrow_semantic_input_allowances() {
        let count_failures = direct_semantic_assembly_failures(
            "crates/ui_components/src/table/mod.rs",
            "fn project(plan: Plan) { let _ = plan.aria_row_count(); }",
        );
        let input_failures = direct_semantic_assembly_failures(
            "crates/ui_components/src/table/editors.rs",
            "fn editor(checkbox: Checkbox) { let _ = checkbox.aria_label(\"label\"); }",
        );

        assert!(count_failures.is_empty(), "{count_failures:#?}");
        assert!(input_failures.is_empty(), "{input_failures:#?}");
    }

    #[test]
    fn public_click_event_audit_rejects_public_surface_shapes() {
        let cases = [
            (
                "builder method",
                r#"
use open_gpui::ClickEvent;
pub struct Control;
impl Control {
    pub fn on_click(self, handler: impl Fn(&ClickEvent)) -> Self {
        let _ = handler;
        self
    }
}
"#,
                "Control::on_click",
            ),
            (
                "free function",
                r#"
use open_gpui::ClickEvent;
pub fn dispatch_raw(event: &ClickEvent) {
    let _ = event;
}
"#,
                "dispatch_raw",
            ),
            (
                "callback alias",
                r#"
use open_gpui::ClickEvent;
pub type RawClickHandler = Box<dyn Fn(&ClickEvent)>;
"#,
                "RawClickHandler",
            ),
            (
                "public field",
                r#"
use open_gpui::ClickEvent;
pub struct CallbackSlot {
    pub handler: Box<dyn Fn(&ClickEvent)>,
}
"#,
                "CallbackSlot::handler",
            ),
            (
                "public enum variant",
                r#"
use open_gpui::ClickEvent;
pub enum CallbackMessage {
    Raw(Box<dyn Fn(&ClickEvent)>),
}
"#,
                "CallbackMessage::Raw",
            ),
            (
                "public trait method bound",
                r#"
use open_gpui::ClickEvent;
pub trait RawActivation {
    fn subscribe<F>(&self, handler: F)
    where
        F: Fn(&ClickEvent);
}
"#,
                "RawActivation::subscribe",
            ),
            (
                "renamed import",
                r#"
use open_gpui::ClickEvent as RawClick;
pub fn dispatch_renamed(event: &RawClick) {
    let _ = event;
}
"#,
                "dispatch_renamed",
            ),
            (
                "private alias chain",
                r#"
use open_gpui::ClickEvent;
type RawClick = ClickEvent;
type RawHandler = Box<dyn Fn(&RawClick)>;
pub fn subscribe(handler: RawHandler) {
    let _ = handler;
}
"#,
                "subscribe",
            ),
            (
                "return type",
                r#"
use open_gpui::ClickEvent;
pub fn raw_event() -> ClickEvent {
    todo!()
}
"#,
                "raw_event",
            ),
            (
                "public generic bound",
                r#"
use open_gpui::ClickEvent;
pub struct GenericCallbackSlot<F: Fn(&ClickEvent)> {
    handler: F,
}
"#,
                "GenericCallbackSlot",
            ),
            (
                "trait supertrait",
                r#"
use open_gpui::ClickEvent;
pub trait RawSupertrait: Fn(&ClickEvent) {}
"#,
                "RawSupertrait",
            ),
            (
                "trait associated type",
                r#"
use open_gpui::ClickEvent;
pub trait RawAssociatedType {
    type Handler: Fn(&ClickEvent);
}
"#,
                "RawAssociatedType::Handler",
            ),
            (
                "trait associated const",
                r#"
use open_gpui::ClickEvent;
pub trait RawAssociatedConst {
    const HANDLER: fn(&ClickEvent);
}
"#,
                "RawAssociatedConst::HANDLER",
            ),
            (
                "public const",
                r#"
use open_gpui::ClickEvent;
pub const RAW_HANDLER: fn(&ClickEvent) = |_| {};
"#,
                "RAW_HANDLER",
            ),
            (
                "public static",
                r#"
use open_gpui::ClickEvent;
pub static RAW_HANDLER: fn(&ClickEvent) = |_| {};
"#,
                "RAW_HANDLER",
            ),
            (
                "public tuple field",
                r#"
use open_gpui::ClickEvent;
pub struct RawTuple(pub fn(&ClickEvent));
"#,
                "RawTuple::0",
            ),
            (
                "public union field",
                r#"
use open_gpui::ClickEvent;
pub union RawUnion {
    pub handler: fn(&ClickEvent),
}
"#,
                "RawUnion::handler",
            ),
        ];

        for (case, source, api) in cases {
            let failures =
                public_click_event_api_failures("crates/ui_components/src/fixture.rs", source);
            assert!(
                has_failure(&failures, api),
                "{case} should reject `{api}`: {failures:#?}"
            );
        }
    }

    #[test]
    fn public_click_event_audit_allows_internal_render_input() {
        let source = r#"
use open_gpui::ClickEvent;

type PrivateRawClick = ClickEvent;
type PrivateRawHandler = Box<dyn Fn(&PrivateRawClick)>;

fn private_callback(handler: impl Fn(&ClickEvent)) {
    let _ = handler;
}

fn private_alias_callback(handler: PrivateRawHandler) {
    let _ = handler;
}

pub(crate) fn crate_callback(handler: impl Fn(&ClickEvent)) {
    let _ = handler;
}

fn render() {
    div().on_click(|_event: &ClickEvent, _window, _cx| {});
}
"#;

        let failures =
            public_click_event_api_failures("crates/ui_components/src/fixture.rs", source);

        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn public_click_event_raw_api_allowlist_is_empty() {
        assert!(PUBLIC_CLICK_EVENT_API_ALLOWLIST.is_empty());
    }

    #[test]
    fn runtime_a11y_evidence_consumer_is_rejected_but_empty_scaffold_is_allowed() {
        let consumer_failures = old_a11y_evidence_failures(
            "crates/ui_components/src/button.rs",
            "fn render() { let _ = COMPONENT_A11Y_EVIDENCE; }",
        );
        let scaffold_failures = old_a11y_evidence_failures(
            "crates/ui_components/src/component_contract/evidence.rs",
            "pub const COMPONENT_A11Y_EVIDENCE: &[ComponentA11yEvidence] = &[];",
        );
        let scaffold_consumer_failures = old_a11y_evidence_failures(
            "crates/ui_components/src/component_contract/evidence.rs",
            "pub const COMPONENT_A11Y_EVIDENCE: &[ComponentA11yEvidence] = &[];\nfn render() { let _ = COMPONENT_A11Y_EVIDENCE; }",
        );

        assert!(has_failure(&consumer_failures, "runtime source consumes"));
        assert!(scaffold_failures.is_empty(), "{scaffold_failures:#?}");
        assert!(has_failure(
            &scaffold_consumer_failures,
            "runtime source consumes"
        ));
    }

    #[test]
    fn contract_row_parser_reads_entry_fields() {
        let source = r#"
pub const COMPONENT_CONTRACT_ROWS: &[ComponentContractEntry] = &[
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

        let (entries, failures) = contract_rows_from_source(source);

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Button");
        assert_eq!(entries[0].owner, "OfficialComponent");
        assert_eq!(entries[0].gallery_status, "OfficialComponent");
        assert_eq!(entries[0].docs_status, DocsStatus::ComponentContract);
        assert_eq!(entries[0].docs_token.as_deref(), Some("Button contract"));
        assert!(entries[0].default_export);
        assert_eq!(entries[0].source_home, "button.rs");
    }

    #[test]
    fn audit_reports_missing_default_export() {
        let entries = [entry(
            "Button",
            "OfficialComponent",
            "OfficialComponent",
            DocsStatus::ComponentCatalog,
            Some("Button"),
            true,
            "button.rs",
        )];
        let root_exports = BTreeSet::new();
        let prelude_exports = BTreeSet::from(["Button".to_string()]);

        let failures = audit_contract_rows(
            &entries,
            &root_exports,
            &prelude_exports,
            &BTreeSet::new(),
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
            "OfficialComponent",
            "OfficialOverlay",
            DocsStatus::ComponentContract,
            Some("Tooltip contract"),
            false,
            "tooltip.rs",
        )];

        let failures = audit_contract_rows(
            &entries,
            &BTreeSet::new(),
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
            "OfficialComponent",
            "OfficialOverlay",
            DocsStatus::ComponentCatalog,
            Some("Dialog"),
            false,
            "dialog.rs",
        )];

        let failures = audit_contract_rows(
            &entries,
            &BTreeSet::new(),
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
            "DeprecatedRemovalTarget",
            "NotInGallery",
            DocsStatus::Verification,
            Some("primitives::overlay"),
            false,
            "removed",
        )];

        let failures = audit_contract_rows(
            &entries,
            &BTreeSet::new(),
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
    fn audit_reports_incompatible_contract_gallery_status() {
        let entries = [entry(
            "ButtonState",
            "RendererNeutralStateContract",
            "OfficialComponent",
            DocsStatus::ComponentContract,
            Some("ButtonState"),
            false,
            "button.rs",
        )];

        let failures = audit_contract_rows(
            &entries,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            &docs("ButtonState", ""),
            |entry| entry.source_home == "button.rs",
            |_| false,
        );

        assert!(has_failure(&failures, "gallery_status `OfficialComponent`"));
    }

    #[test]
    fn verification_docs_inventory_fallback_is_removed_only() {
        let entries = [entry(
            "VerificationOnly",
            "RendererNeutralStateContract",
            "NotInGallery",
            DocsStatus::Verification,
            Some("missing-verification-token"),
            false,
            "verification.rs",
        )];

        let failures = audit_contract_rows(
            &entries,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            &docs("", "primitive_deletion_target_inventory"),
            |entry| entry.source_home == "verification.rs",
            |_| false,
        );

        assert!(has_failure(&failures, "missing-verification-token"));
    }

    #[test]
    fn audit_allows_root_default_export_outside_common_prelude() {
        let entries = [entry(
            "TableGlobalFilter",
            "OfficialComponentRecipe",
            "NotInGallery",
            DocsStatus::ComponentCatalog,
            Some("TableGlobalFilter"),
            true,
            "table/global_filter",
        )];
        let root_exports = BTreeSet::from(["TableGlobalFilter".to_string()]);
        let prelude_exports = BTreeSet::new();
        let common_exports = BTreeSet::new();

        let failures = audit_contract_rows(
            &entries,
            &root_exports,
            &prelude_exports,
            &common_exports,
            &docs("", ""),
            |entry| entry.source_home == "table/global_filter",
            |_| false,
        );

        assert_eq!(failures, Vec::<String>::new());
    }

    #[test]
    fn audit_reports_common_default_missing_from_prelude() {
        let entries = [entry(
            "Button",
            "OfficialComponent",
            "OfficialComponent",
            DocsStatus::ComponentCatalog,
            Some("Button"),
            true,
            "button.rs",
        )];
        let root_exports = BTreeSet::from(["Button".to_string()]);
        let prelude_exports = BTreeSet::new();
        let common_exports = BTreeSet::from(["Button".to_string()]);

        let failures = audit_contract_rows(
            &entries,
            &root_exports,
            &prelude_exports,
            &common_exports,
            &docs("", ""),
            |entry| entry.source_home == "button.rs",
            |_| false,
        );

        assert!(has_failure(&failures, "common contract row `Button`"));
        assert!(has_failure(&failures, "prelude exports"));
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

    #[test]
    fn audit_reports_missing_non_executable_conformance_evidence_token() {
        let gates = [ConformanceGate {
            id: "theme-schema".to_string(),
            evidence: BTreeSet::from(["crates/ui_components/src/theme/schema.rs".to_string()]),
        }];

        let failures = audit_conformance_gate_evidence(&gates);

        assert!(has_failure(
            &failures,
            "cargo run -p xtask -- scan-theme-schema"
        ));
    }

    #[test]
    fn audit_does_not_treat_a11y_display_tokens_as_executable_evidence() {
        let gates = [ConformanceGate {
            id: "a11y-labels".to_string(),
            evidence: BTreeSet::new(),
        }];

        let failures = audit_conformance_gate_evidence(&gates);

        for display_token in ["SemanticDescriptor", "ComponentA11yContract", "TreeUpdate"] {
            assert!(
                !has_failure(&failures, display_token),
                "display token `{display_token}` must not substitute for executable evidence"
            );
        }
    }
}
