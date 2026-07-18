use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use open_gpui_ui_components::component_contract::{
    COMPONENT_CONTRACT_GLOBAL_SCENARIOS, COMPONENT_CONTRACT_ROWS, ComponentContractEntry,
    PublicApiExport, PublicApiTier, common_public_exports, default_public_exports,
    diagnostic_public_exports,
};
use serde::Deserialize;

const SCENARIO_ARTIFACT_SUFFIX: &str = ".scenarios.toml";
const DOC_PROJECTION_BEGIN: &str = "<!-- BEGIN COMPONENT CONTRACT PROJECTION -->";
const DOC_PROJECTION_END: &str = "<!-- END COMPONENT CONTRACT PROJECTION -->";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioArtifact {
    schema: u16,
    scenario: Vec<ScenarioDeclaration>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioDeclaration {
    id: String,
    contracts: Vec<String>,
    test: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScenarioRegistration {
    id: String,
    contracts: BTreeSet<String>,
    package: String,
    target: String,
    test: String,
    owner_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactTestCommand {
    program: &'static str,
    args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContractRow {
    id: &'static str,
    revision: u16,
    family: &'static str,
    required_scenarios: &'static [&'static str],
}

impl From<&ComponentContractEntry> for ContractRow {
    fn from(entry: &ComponentContractEntry) -> Self {
        Self {
            id: entry.id().as_str(),
            revision: entry.revision().value(),
            family: entry.family().as_str(),
            required_scenarios: entry.required_scenarios(),
        }
    }
}

fn canonical_contract_rows() -> Vec<ContractRow> {
    COMPONENT_CONTRACT_ROWS
        .iter()
        .map(ContractRow::from)
        .collect()
}

pub(crate) fn scan_ui_contract(root: &Path) -> Result<(), ()> {
    println!("==> scan federated UI contract");

    let contract_rows = canonical_contract_rows();
    let (registrations, mut failures) = discover_scenario_registrations(root);
    failures.extend(contract_metadata_failures(&contract_rows));
    failures.extend(public_export_failures(
        &contract_rows,
        common_public_exports(),
        default_public_exports(),
        diagnostic_public_exports(),
    ));
    failures.extend(scenario_binding_failures(
        &contract_rows,
        COMPONENT_CONTRACT_GLOBAL_SCENARIOS,
        &registrations,
    ));
    failures.extend(documentation_projection_failures(root));
    failures.extend(old_authority_residue_failures(root));

    if !failures.is_empty() {
        eprintln!("Federated UI contract scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        return Err(());
    }

    println!("==> run native scenario registrations");
    let mut execution_failures = Vec::new();
    for registration in &registrations {
        let command = exact_test_command(registration);
        let args = command.args.iter().map(String::as_str).collect::<Vec<_>>();
        if crate::commands::run(root, command.program, &args).is_err() {
            execution_failures.push(format!(
                "{}: scenario `{}` failed at {}::{}::{}",
                registration.owner_path,
                registration.id,
                registration.package,
                registration.target,
                registration.test,
            ));
        }
    }

    if execution_failures.is_empty() {
        println!("Federated UI contract scan passed");
        Ok(())
    } else {
        eprintln!("Native UI scenarios failed:");
        for failure in execution_failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn contract_metadata_failures(rows: &[ContractRow]) -> Vec<String> {
    let mut failures = Vec::new();
    let mut owners = BTreeMap::new();

    for row in rows {
        let id = row.id;
        if id.trim().is_empty() {
            failures.push("component contract id cannot be empty".to_owned());
        }
        if row.revision == 0 {
            failures.push(format!("component contract `{id}` has zero revision"));
        }
        if row.family.trim().is_empty() {
            failures.push(format!("component contract `{id}` has an empty family"));
        }
        if let Some(previous) = owners.insert(id, row.revision) {
            failures.push(format!(
                "component contract `{id}` is duplicated at revisions {previous} and {}",
                row.revision,
            ));
        }

        let mut scenarios = BTreeSet::new();
        for scenario in row.required_scenarios {
            if scenario.trim().is_empty() {
                failures.push(format!(
                    "component contract `{id}` requires an empty scenario id"
                ));
            } else if !scenarios.insert(*scenario) {
                failures.push(format!(
                    "component contract `{id}` repeats required scenario `{scenario}`",
                ));
            }
        }
    }

    if rows.len() != 48 {
        failures.push(format!(
            "component contract must contain the 48 official components, found {}",
            rows.len(),
        ));
    }

    failures
}

fn public_export_failures<'a>(
    rows: &[ContractRow],
    common: impl IntoIterator<Item = &'a PublicApiExport>,
    default: impl IntoIterator<Item = &'a PublicApiExport>,
    diagnostic: impl IntoIterator<Item = &'a PublicApiExport>,
) -> Vec<String> {
    let mut failures = Vec::new();
    let common = export_map("common", common, &mut failures);
    let default = export_map("default", default, &mut failures);
    let diagnostic = export_map("diagnostic", diagnostic, &mut failures);

    for row in rows {
        let id = row.id;
        if !common.contains_key(id) {
            failures.push(format!(
                "component contract `{id}` is missing common export owner `open_gpui_ui_components::{{root,common,prelude}}`",
            ));
        }
        if !default.contains_key(id) {
            failures.push(format!(
                "component contract `{id}` is missing default export owner `open_gpui_ui_components::root`",
            ));
        }
    }

    for (name, export) in &diagnostic {
        if let Some(leaked) = common.get(name).or_else(|| default.get(name)) {
            failures.push(format!(
                "diagnostic export `{name}` owner `{}` leaked through `{}`",
                export.owner(),
                leaked.owner(),
            ));
        }
    }

    match diagnostic.get("TableBehaviorSnapshot") {
        Some(export) if export.owner() == "open_gpui_ui_components::table" => {}
        Some(export) => failures.push(format!(
            "TableBehaviorSnapshot diagnostic owner drifted to `{}`",
            export.owner(),
        )),
        None => failures.push(
            "TableBehaviorSnapshot is missing its explicit table diagnostic owner".to_owned(),
        ),
    }
    if !default.contains_key("TableVirtualizerSnapshot") {
        failures
            .push("TableVirtualizerSnapshot must remain a default restoration input".to_owned());
    }

    failures
}

pub(crate) fn ui_component_public_export_failures() -> Vec<String> {
    public_export_failures(
        &canonical_contract_rows(),
        common_public_exports(),
        default_public_exports(),
        diagnostic_public_exports(),
    )
}

fn export_map<'a>(
    label: &str,
    exports: impl IntoIterator<Item = &'a PublicApiExport>,
    failures: &mut Vec<String>,
) -> BTreeMap<&'a str, &'a PublicApiExport> {
    let mut result = BTreeMap::new();
    for export in exports {
        if let Some(previous) = result.insert(export.name(), export) {
            failures.push(format!(
                "{label} export `{}` has duplicate owners `{}` and `{}`",
                export.name(),
                previous.owner(),
                export.owner(),
            ));
        }
        let tier_allowed = match label {
            "common" => export.tier() == PublicApiTier::Common,
            "default" => matches!(
                export.tier(),
                PublicApiTier::Common | PublicApiTier::Extended
            ),
            "diagnostic" => export.tier() == PublicApiTier::Diagnostic,
            _ => false,
        };
        if !tier_allowed {
            failures.push(format!(
                "{label} export `{}` owner `{}` has tier {:?}",
                export.name(),
                export.owner(),
                export.tier(),
            ));
        }
        let expected_owner = match export.tier() {
            PublicApiTier::Common => "open_gpui_ui_components::{root,common,prelude}",
            PublicApiTier::Extended => "open_gpui_ui_components::root",
            PublicApiTier::Diagnostic => "open_gpui_ui_components::table",
        };
        if export.owner() != expected_owner {
            failures.push(format!(
                "{label} export `{}` has owner `{}` instead of `{expected_owner}`",
                export.name(),
                export.owner(),
            ));
        }
    }
    result
}

fn discover_scenario_registrations(root: &Path) -> (Vec<ScenarioRegistration>, Vec<String>) {
    let mut paths = Vec::new();
    let mut failures = Vec::new();
    collect_named_files(root, SCENARIO_ARTIFACT_SUFFIX, &mut paths, &mut failures);
    paths.sort();

    let all_contracts = COMPONENT_CONTRACT_ROWS
        .iter()
        .map(|row| row.id().as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let mut registrations = Vec::new();

    for path in paths {
        let owner_path = repo_relative_path(root, &path);
        let Some(tests_dir) = path.parent() else {
            failures.push(format!(
                "{owner_path}: scenario artifact has no tests directory"
            ));
            continue;
        };
        if tests_dir.file_name().and_then(|name| name.to_str()) != Some("tests") {
            failures.push(format!(
                "{owner_path}: scenario artifact must live directly under a package tests directory",
            ));
            continue;
        }
        let Some(package_dir) = tests_dir.parent() else {
            failures.push(format!(
                "{owner_path}: scenario artifact has no package owner"
            ));
            continue;
        };
        let Some(package) = package_name(&package_dir.join("Cargo.toml"), &mut failures) else {
            continue;
        };
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            failures.push(format!(
                "{owner_path}: scenario artifact has no UTF-8 file name"
            ));
            continue;
        };
        let Some(target) = file_name.strip_suffix(SCENARIO_ARTIFACT_SUFFIX) else {
            continue;
        };
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!("{owner_path}: failed to read artifact: {error}"));
                continue;
            }
        };
        let artifact = match toml::from_str::<ScenarioArtifact>(&source) {
            Ok(artifact) => artifact,
            Err(error) => {
                failures.push(format!("{owner_path}: invalid scenario artifact: {error}"));
                continue;
            }
        };
        if artifact.schema != 1 {
            failures.push(format!(
                "{owner_path}: unsupported scenario schema {}; expected 1",
                artifact.schema,
            ));
            continue;
        }

        for declaration in artifact.scenario {
            let contracts =
                declared_contracts(&declaration, &all_contracts, &owner_path, &mut failures);
            registrations.push(ScenarioRegistration {
                id: declaration.id,
                contracts,
                package: package.clone(),
                target: target.to_owned(),
                test: declaration.test,
                owner_path: owner_path.clone(),
            });
        }
    }

    (registrations, failures)
}

fn scenario_binding_failures(
    rows: &[ContractRow],
    global_scenarios: &[&str],
    registrations: &[ScenarioRegistration],
) -> Vec<String> {
    let mut failures = Vec::new();
    let contract_ids = rows.iter().map(|row| row.id).collect::<BTreeSet<_>>();
    let mut expected = BTreeMap::<&str, BTreeSet<&str>>::new();
    for global in global_scenarios {
        expected.entry(global).or_default().extend(&contract_ids);
    }
    for row in rows {
        for scenario in row.required_scenarios {
            expected.entry(scenario).or_default().insert(row.id);
        }
    }

    let mut actual = BTreeMap::<&str, &ScenarioRegistration>::new();
    let mut coordinates = BTreeMap::<(&str, &str, &str), &ScenarioRegistration>::new();
    for registration in registrations {
        if registration.id.trim().is_empty() {
            failures.push(format!(
                "{}: scenario id cannot be empty",
                registration.owner_path,
            ));
            continue;
        }
        if !is_valid_test_coordinate(&registration.test) {
            failures.push(format!(
                "{}: scenario `{}` test coordinate `{}` is not an exact ASCII Rust test path",
                registration.owner_path, registration.id, registration.test,
            ));
        }
        if let Some(previous) = actual.insert(&registration.id, registration) {
            failures.push(format!(
                "scenario `{}` is duplicated by `{}` ({}::{}) and `{}` ({}::{})",
                registration.id,
                previous.owner_path,
                previous.target,
                previous.test,
                registration.owner_path,
                registration.target,
                registration.test,
            ));
        }
        let coordinate = (
            registration.package.as_str(),
            registration.target.as_str(),
            registration.test.as_str(),
        );
        if let Some(previous) = coordinates.insert(coordinate, registration) {
            failures.push(format!(
                "scenario `{}` and scenario `{}` share executable coordinate {} --test {} test(={})",
                previous.id,
                registration.id,
                registration.package,
                registration.target,
                registration.test,
            ));
        }
        for contract in &registration.contracts {
            if !contract_ids.contains(contract.as_str()) {
                failures.push(format!(
                    "{}: scenario `{}` references unknown component contract `{contract}`",
                    registration.owner_path, registration.id,
                ));
            }
        }
    }

    for (scenario, expected_contracts) in &expected {
        let Some(registration) = actual.get(scenario) else {
            for contract in expected_contracts {
                failures.push(format!(
                    "component contract `{contract}` requires scenario `{scenario}` but no test-side registration exists",
                ));
            }
            continue;
        };
        let expected_owned = expected_contracts
            .iter()
            .map(|contract| (*contract).to_owned())
            .collect::<BTreeSet<_>>();
        if registration.contracts != expected_owned {
            failures.push(format!(
                "{}: scenario `{scenario}` owner drift; expected contracts {:?}, registered contracts {:?}",
                registration.owner_path, expected_contracts, registration.contracts,
            ));
        }
    }
    for (scenario, registration) in actual {
        if !expected.contains_key(scenario) {
            failures.push(format!(
                "{}: scenario `{scenario}` has no typed component requirement",
                registration.owner_path,
            ));
        }
    }

    failures
}

fn exact_test_command(registration: &ScenarioRegistration) -> ExactTestCommand {
    assert!(
        is_valid_test_coordinate(&registration.test),
        "scenario test coordinates must be validated before command construction"
    );
    ExactTestCommand {
        program: "cargo",
        args: [
            "nextest".to_owned(),
            "run".to_owned(),
            "--locked".to_owned(),
            "-p".to_owned(),
            registration.package.clone(),
            "--test".to_owned(),
            registration.target.clone(),
            "--ignore-default-filter".to_owned(),
            "--run-ignored".to_owned(),
            "default".to_owned(),
            "--no-tests".to_owned(),
            "fail".to_owned(),
            "-E".to_owned(),
            format!("test(={})", registration.test),
        ]
        .into_iter()
        .collect(),
    }
}

fn declared_contracts(
    declaration: &ScenarioDeclaration,
    all_contracts: &BTreeSet<String>,
    owner_path: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    if declaration.contracts.as_slice() == ["*"] {
        return all_contracts.clone();
    }
    if declaration.contracts.is_empty() {
        failures.push(format!(
            "{owner_path}: scenario `{}` must bind at least one component contract",
            declaration.id,
        ));
    }

    let mut contracts = BTreeSet::new();
    for contract in &declaration.contracts {
        if contract == "*" {
            failures.push(format!(
                "{owner_path}: scenario `{}` wildcard must be the only contract binding",
                declaration.id,
            ));
            continue;
        }
        if contract.trim().is_empty() {
            failures.push(format!(
                "{owner_path}: scenario `{}` contains an empty component contract binding",
                declaration.id,
            ));
            continue;
        }
        if !contracts.insert(contract.clone()) {
            failures.push(format!(
                "{owner_path}: scenario `{}` repeats component contract `{contract}`",
                declaration.id,
            ));
        }
    }
    contracts
}

fn is_valid_test_coordinate(test: &str) -> bool {
    let mut segments = test.split("::").peekable();
    if segments.peek().is_none() {
        return false;
    }
    segments.all(|segment| {
        let mut characters = segment.chars();
        characters
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
            && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    })
}

fn documentation_projection_failures(root: &Path) -> Vec<String> {
    let path = root.join("docs/ui/component-contract.md");
    let label = repo_relative_path(root, &path);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => return vec![format!("{label}: failed to read: {error}")],
    };
    let Some(start) = source.find(DOC_PROJECTION_BEGIN) else {
        return vec![format!("{label}: missing `{DOC_PROJECTION_BEGIN}`")];
    };
    let remaining = &source[start + DOC_PROJECTION_BEGIN.len()..];
    let Some(end) = remaining.find(DOC_PROJECTION_END) else {
        return vec![format!("{label}: missing `{DOC_PROJECTION_END}`")];
    };
    let projection = parse_documentation_projection(&remaining[..end]);
    let expected = COMPONENT_CONTRACT_ROWS
        .iter()
        .map(|row| {
            (
                row.id().as_str().to_owned(),
                (row.revision().value(), row.family().as_str().to_owned()),
            )
        })
        .collect::<BTreeMap<_, _>>();

    match projection {
        Ok(actual) if actual == expected => Vec::new(),
        Ok(actual) => vec![format!(
            "{label}: component contract projection drifted; expected {expected:?}, actual {actual:?}",
        )],
        Err(error) => vec![format!("{label}: {error}")],
    }
}

fn parse_documentation_projection(source: &str) -> Result<BTreeMap<String, (u16, String)>, String> {
    let mut rows = BTreeMap::new();
    for line in source.lines().map(str::trim) {
        if !line.starts_with('|') {
            continue;
        }
        let fields = line
            .trim_matches('|')
            .split('|')
            .map(|field| field.trim().trim_matches('`'))
            .collect::<Vec<_>>();
        if fields.len() != 3 || fields[0] == "Contract ID" || fields[0].starts_with('-') {
            continue;
        }
        let revision = fields[1]
            .parse::<u16>()
            .map_err(|error| format!("invalid revision for `{}`: {error}", fields[0]))?;
        if rows
            .insert(fields[0].to_owned(), (revision, fields[2].to_owned()))
            .is_some()
        {
            return Err(format!("duplicate documentation contract `{}`", fields[0]));
        }
    }
    Ok(rows)
}

fn old_authority_residue_failures(root: &Path) -> Vec<String> {
    let tokens = [
        "COMPONENT_API_INVENTORY",
        "ComponentApiInventoryEntry",
        "COMPONENT_A11Y_EVIDENCE",
        "ComponentA11yEvidence",
        "COMPONENT_CONFORMANCE_GATES",
        "ComponentConformanceGate",
        "PUBLIC_SURFACE_OWNER_MAP",
        "PublicSurfaceOwnerEntry",
        "component_source_inputs",
    ];
    let mut failures = Vec::new();
    for relative in [
        "crates/ui_components/src",
        "examples/ui-foundation-gallery/src",
    ] {
        let mut files = Vec::new();
        collect_rust_files(&root.join(relative), &mut files, &mut failures);
        for path in files {
            let label = repo_relative_path(root, &path);
            let source = match fs::read_to_string(&path) {
                Ok(source) => source,
                Err(error) => {
                    failures.push(format!("{label}: failed to read: {error}"));
                    continue;
                }
            };
            for token in tokens {
                if source.contains(token) {
                    failures.push(format!(
                        "{label}: removed centralized authority `{token}` remains in production source",
                    ));
                }
            }
        }
    }
    failures
}

fn collect_named_files(
    directory: &Path,
    suffix: &str,
    files: &mut Vec<PathBuf>,
    failures: &mut Vec<String>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            failures.push(format!(
                "{}: failed to read directory: {error}",
                directory.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(format!(
                    "{}: failed to read entry: {error}",
                    directory.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            if matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some(".git" | "target" | "repo-ref")
            ) {
                continue;
            }
            collect_named_files(&path, suffix, files, failures);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
        {
            files.push(path);
        }
    }
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>, failures: &mut Vec<String>) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            failures.push(format!(
                "{}: failed to read directory: {error}",
                directory.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(format!(
                    "{}: failed to read entry: {error}",
                    directory.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files, failures);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn package_name(path: &Path, failures: &mut Vec<String>) -> Option<String> {
    let source = fs::read_to_string(path)
        .map_err(|error| {
            failures.push(format!(
                "{}: failed to read package manifest: {error}",
                path.display()
            ));
        })
        .ok()?;
    let manifest = toml::from_str::<toml::Value>(&source)
        .map_err(|error| {
            failures.push(format!(
                "{}: invalid package manifest: {error}",
                path.display()
            ));
        })
        .ok()?;
    manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            failures.push(format!("{}: package.name is missing", path.display()));
            None
        })
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract_row(id: &'static str, required_scenarios: &'static [&'static str]) -> ContractRow {
        ContractRow {
            id,
            revision: 1,
            family: "test",
            required_scenarios,
        }
    }

    fn registration(id: &str, contracts: &[&str], owner_path: &str) -> ScenarioRegistration {
        ScenarioRegistration {
            id: id.to_owned(),
            contracts: contracts.iter().map(|value| (*value).to_owned()).collect(),
            package: "package".to_owned(),
            target: "target".to_owned(),
            test: "test".to_owned(),
            owner_path: owner_path.to_owned(),
        }
    }

    #[test]
    fn scenario_validator_reports_missing_duplicate_unknown_and_owner_drift() {
        let rows = [
            contract_row("Button", &["button.activate"]),
            contract_row("Dialog", &[]),
        ];
        let registrations = [
            registration("global", &["Button"], "tests/first.scenarios.toml"),
            registration("global", &["Button"], "tests/second.scenarios.toml"),
            registration("orphan", &["Missing"], "tests/orphan.scenarios.toml"),
        ];

        let failures = scenario_binding_failures(&rows, &["global"], &registrations).join("\n");
        assert!(failures.contains("duplicated by"));
        assert!(failures.contains("owner drift"));
        assert!(failures.contains("requires scenario `button.activate`"));
        assert!(failures.contains("unknown component contract `Missing`"));
        assert!(failures.contains("scenario `orphan` has no typed component requirement"));
        assert!(failures.contains("tests/first.scenarios.toml"));
        assert!(failures.contains("tests/second.scenarios.toml"));
    }

    #[test]
    fn documentation_projection_parser_is_structured_and_rejects_duplicates() {
        let projection = parse_documentation_projection(
            "| Contract ID | Revision | Family |\n| --- | ---: | --- |\n| `Button` | 1 | `action` |",
        )
        .unwrap();
        assert_eq!(projection.get("Button"), Some(&(1, "action".to_owned())));

        let duplicate = parse_documentation_projection(
            "| `Button` | 1 | `action` |\n| `Button` | 2 | `action` |",
        )
        .unwrap_err();
        assert!(duplicate.contains("duplicate documentation contract `Button`"));
    }

    #[test]
    fn exact_test_command_preserves_native_nextest_isolation() {
        let registration =
            registration("button.activate", &["Button"], "tests/a11y.scenarios.toml");
        let command = exact_test_command(&registration);
        assert_eq!(command.program, "cargo");
        assert!(
            command
                .args
                .windows(2)
                .any(|args| args == ["--test", "target"])
        );
        assert_eq!(command.args.last().unwrap(), "test(=test)");
    }

    #[test]
    fn scenario_declarations_reject_duplicate_contracts_and_filter_expressions() {
        let declaration = ScenarioDeclaration {
            id: "button.activate".to_owned(),
            contracts: vec!["Button".to_owned(), "Button".to_owned()],
            test: "button_works".to_owned(),
        };
        let mut failures = Vec::new();
        let contracts = declared_contracts(
            &declaration,
            &BTreeSet::from(["Button".to_owned()]),
            "tests/a11y.scenarios.toml",
            &mut failures,
        );
        assert_eq!(contracts, BTreeSet::from(["Button".to_owned()]));
        assert!(
            failures
                .join("\n")
                .contains("repeats component contract `Button`")
        );

        assert!(is_valid_test_coordinate("module::nested::button_works"));
        assert!(!is_valid_test_coordinate("missing) | all("));
        assert!(!is_valid_test_coordinate("module::::button_works"));
    }

    #[test]
    fn scenario_validator_rejects_reused_executable_coordinates() {
        let rows = [contract_row(
            "Button",
            &["button.activate", "button.keyboard"],
        )];
        let registrations = [
            registration("button.activate", &["Button"], "tests/first.scenarios.toml"),
            registration(
                "button.keyboard",
                &["Button"],
                "tests/second.scenarios.toml",
            ),
        ];

        let failures = scenario_binding_failures(&rows, &[], &registrations).join("\n");
        assert!(failures.contains("share executable coordinate"));
    }
}
