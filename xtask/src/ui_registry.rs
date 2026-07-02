use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;

const REGISTRY_ARTIFACT: &str = "docs/registry/open-gpui-component-registry-v1.json";
const SCHEMA_ARTIFACT: &str = "docs/schemas/open-gpui-component-registry-v1.schema.json";
const EXPORT_REGISTRY_COMMAND: &[&str] = &[
    "run",
    "-p",
    "open-gpui-ui-components",
    "--example",
    "export_component_registry",
    "--quiet",
];
const EXPORT_SCHEMA_COMMAND: &[&str] = &[
    "run",
    "-p",
    "open-gpui-ui-components",
    "--example",
    "export_component_registry_schema",
    "--quiet",
];

const REQUIRED_REGISTRY_ENTRIES: &[&str] = &[
    "Button",
    "Command",
    "Table",
    "TableFacetedFilter",
    "ThemeDefinition",
    "TextInputController",
    "primitives::overlay",
];

const REQUIRED_SCHEMA_MARKERS: &[&str] = &[
    "schema_version",
    "package",
    "entries",
    "recipes",
    "distribution_authority",
    "official_component",
    "official_component_recipe",
    "renderer_neutral_state_contract",
    "gpui_adapter_helper",
    "deprecated_removal_target",
    "source_components",
    "generated_files",
    "verification_gates",
    "app_owned_source",
    "cargo_dependency_snippet",
    "gallery_story_sample",
];

pub(crate) fn scan_ui_registry(root: &Path) -> Result<(), ()> {
    println!("==> scan UI registry");

    let failures = ui_registry_failures(root);
    if failures.is_empty() {
        println!("UI registry scan passed");
        Ok(())
    } else {
        eprintln!("UI registry scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

pub(crate) fn ui_registry_failures(root: &Path) -> Vec<String> {
    let registry_path = root.join(REGISTRY_ARTIFACT);
    let schema_path = root.join(SCHEMA_ARTIFACT);

    let registry_source = match fs::read_to_string(&registry_path) {
        Ok(source) => source,
        Err(error) => {
            return vec![format!(
                "{REGISTRY_ARTIFACT}: failed to read component registry artifact: {error}"
            )];
        }
    };
    let schema_source = match fs::read_to_string(&schema_path) {
        Ok(source) => source,
        Err(error) => {
            return vec![format!(
                "{SCHEMA_ARTIFACT}: failed to read component registry schema artifact: {error}"
            )];
        }
    };

    let mut failures = match generated_registry(root) {
        Ok(generated_source) => {
            registry_artifact_failures_for_sources(&registry_source, &generated_source)
        }
        Err(error) => vec![error],
    };

    failures.extend(match generated_schema(root) {
        Ok(generated_source) => {
            schema_artifact_failures_for_sources(&schema_source, &generated_source)
        }
        Err(error) => vec![error],
    });

    failures.extend(registry_manifest_shape_failures(&registry_source));
    failures.extend(required_schema_marker_failures(&schema_source));
    failures
}

fn generated_registry(root: &Path) -> Result<String, String> {
    generated_cargo_output(root, EXPORT_REGISTRY_COMMAND, "component registry export")
}

fn generated_schema(root: &Path) -> Result<String, String> {
    generated_cargo_output(
        root,
        EXPORT_SCHEMA_COMMAND,
        "component registry schema export",
    )
}

fn generated_cargo_output(root: &Path, args: &[&str], label: &str) -> Result<String, String> {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("cargo {}: failed to run: {error}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cargo {}: {label} failed: {}",
            args.join(" "),
            stderr.trim()
        ));
    }

    String::from_utf8(output.stdout)
        .map_err(|error| format!("cargo {}: emitted invalid UTF-8: {error}", args.join(" ")))
}

fn registry_artifact_failures_for_sources(
    artifact_source: &str,
    generated_source: &str,
) -> Vec<String> {
    let mut failures = Vec::new();
    if let (Some(artifact), Some(generated)) = (
        normalized_json(artifact_source, REGISTRY_ARTIFACT, &mut failures),
        normalized_json(
            generated_source,
            "component_registry_manifest()",
            &mut failures,
        ),
    ) {
        if artifact != generated {
            failures.push(format!(
                "{REGISTRY_ARTIFACT}: component_registry_manifest() drifted from the committed artifact; regenerate with `cargo run -p open-gpui-ui-components --example export_component_registry --quiet` and replace this file"
            ));
        }
    }

    failures.extend(missing_generated_items(
        artifact_source,
        generated_source,
        "/entries",
        "name",
        "manifest entry",
    ));
    failures.extend(missing_generated_items(
        artifact_source,
        generated_source,
        "/recipes",
        "id",
        "scaffold recipe",
    ));

    failures
}

fn schema_artifact_failures_for_sources(
    artifact_source: &str,
    generated_source: &str,
) -> Vec<String> {
    let mut failures = Vec::new();
    if let (Some(artifact), Some(generated)) = (
        normalized_json(artifact_source, SCHEMA_ARTIFACT, &mut failures),
        normalized_json(
            generated_source,
            "component_registry_manifest_schema()",
            &mut failures,
        ),
    ) {
        if artifact != generated {
            failures.push(format!(
                "{SCHEMA_ARTIFACT}: component_registry_manifest_schema() drifted from the committed artifact; regenerate with `cargo run -p open-gpui-ui-components --example export_component_registry_schema --quiet` and replace this file"
            ));
        }
    }

    failures
}

fn registry_manifest_shape_failures(source: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let Some(manifest) = parse_json_value(source, REGISTRY_ARTIFACT, &mut failures) else {
        return failures;
    };

    let entry_names = registry_entry_names(&manifest, &mut failures);
    for required in REQUIRED_REGISTRY_ENTRIES {
        if !entry_names.contains(*required) {
            failures.push(format!(
                "{REGISTRY_ARTIFACT}: component registry manifest is missing required row `{required}`"
            ));
        }
    }

    failures.extend(recipe_reference_failures(&manifest, &entry_names));
    failures
}

fn missing_generated_items(
    artifact_source: &str,
    generated_source: &str,
    array_pointer: &str,
    field: &str,
    label: &str,
) -> Vec<String> {
    let mut failures = Vec::new();
    let Some(artifact) = parse_json_value(artifact_source, REGISTRY_ARTIFACT, &mut failures) else {
        return failures;
    };
    let Some(generated) = parse_json_value(
        generated_source,
        "component_registry_manifest()",
        &mut failures,
    ) else {
        return failures;
    };

    let artifact_names = string_field_set(&artifact, array_pointer, field, &mut failures);
    let generated_names = string_field_set(&generated, array_pointer, field, &mut failures);
    for name in generated_names.difference(&artifact_names) {
        failures.push(format!(
            "{REGISTRY_ARTIFACT}: committed artifact is missing {label} `{name}`"
        ));
    }
    failures
}

fn string_field_set(
    value: &Value,
    array_pointer: &str,
    field: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(items) = value.pointer(array_pointer).and_then(Value::as_array) else {
        failures.push(format!(
            "{REGISTRY_ARTIFACT}: missing array at `{array_pointer}`"
        ));
        return BTreeSet::new();
    };

    items
        .iter()
        .filter_map(|item| {
            item.get(field)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn registry_entry_names(manifest: &Value, failures: &mut Vec<String>) -> BTreeSet<String> {
    let Some(entries) = manifest.pointer("/entries").and_then(Value::as_array) else {
        failures.push(format!("{REGISTRY_ARTIFACT}: missing `entries` array"));
        return BTreeSet::new();
    };

    let mut names = BTreeSet::new();
    for entry in entries {
        match entry.pointer("/name").and_then(Value::as_str) {
            Some(name) => {
                if !names.insert(name.to_owned()) {
                    failures.push(format!(
                        "{REGISTRY_ARTIFACT}: duplicate registry entry `{name}`"
                    ));
                }
            }
            None => failures.push(format!(
                "{REGISTRY_ARTIFACT}: registry entry is missing string `name`: {entry}"
            )),
        }
    }
    names
}

fn recipe_reference_failures(manifest: &Value, entry_names: &BTreeSet<String>) -> Vec<String> {
    let mut failures = Vec::new();
    let Some(recipes) = manifest.pointer("/recipes").and_then(Value::as_array) else {
        failures.push(format!("{REGISTRY_ARTIFACT}: missing `recipes` array"));
        return failures;
    };

    let mut recipe_ids = BTreeSet::new();
    for recipe in recipes {
        let id = recipe
            .pointer("/id")
            .and_then(Value::as_str)
            .unwrap_or("<missing recipe id>");
        if id == "<missing recipe id>" {
            failures.push(format!(
                "{REGISTRY_ARTIFACT}: recipe is missing string `id`"
            ));
        } else if !recipe_ids.insert(id.to_owned()) {
            failures.push(format!("{REGISTRY_ARTIFACT}: duplicate recipe id `{id}`"));
        }

        if !non_empty_array(recipe, "/generated_files") {
            failures.push(format!(
                "{REGISTRY_ARTIFACT}: recipe `{id}` must declare at least one generated file intent"
            ));
        }
        if !non_empty_array(recipe, "/verification_gates") {
            failures.push(format!(
                "{REGISTRY_ARTIFACT}: recipe `{id}` must declare at least one verification gate"
            ));
        }

        let Some(source_components) = recipe
            .pointer("/source_components")
            .and_then(Value::as_array)
        else {
            failures.push(format!(
                "{REGISTRY_ARTIFACT}: recipe `{id}` is missing `source_components`"
            ));
            continue;
        };
        for source_component in source_components {
            match source_component.as_str() {
                Some(name) if entry_names.contains(name) => {}
                Some(name) => failures.push(format!(
                    "{REGISTRY_ARTIFACT}: recipe `{id}` references missing registry row `{name}`"
                )),
                None => failures.push(format!(
                    "{REGISTRY_ARTIFACT}: recipe `{id}` contains a non-string source component `{source_component}`"
                )),
            }
        }
    }

    failures
}

fn non_empty_array(value: &Value, pointer: &str) -> bool {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty())
}

fn required_schema_marker_failures(schema_source: &str) -> Vec<String> {
    REQUIRED_SCHEMA_MARKERS
        .iter()
        .filter(|marker| !schema_source.contains(**marker))
        .map(|marker| {
            format!(
                "{SCHEMA_ARTIFACT}: missing current component registry schema marker `{marker}`"
            )
        })
        .collect()
}

fn normalized_json(source: &str, label: &str, failures: &mut Vec<String>) -> Option<String> {
    let value = parse_json_value(source, label, failures)?;

    serde_json::to_string_pretty(&value)
        .map(|normalized| format!("{normalized}\n"))
        .map_err(|error| {
            failures.push(format!("{label}: failed to normalize JSON: {error}"));
        })
        .ok()
}

fn parse_json_value(source: &str, label: &str, failures: &mut Vec<String>) -> Option<Value> {
    match serde_json::from_str::<Value>(source) {
        Ok(value) => Some(value),
        Err(error) => {
            failures.push(format!("{label}: failed to parse JSON: {error}"));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_failure(failures: &[String], needle: &str) -> bool {
        failures.iter().any(|failure| failure.contains(needle))
    }

    fn minimal_manifest(entries: &[&str], recipes: &str) -> String {
        let entries = entries
            .iter()
            .map(|name| format!(r#"{{"name":"{name}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"schema_version":1,"entries":[{entries}],"recipes":{recipes}}}"#)
    }

    #[test]
    fn registry_artifact_drift_reports_path_and_regeneration_command() {
        let artifact = minimal_manifest(&["Button"], "[]");
        let generated = minimal_manifest(&["Button", "Command"], "[]");

        let failures = registry_artifact_failures_for_sources(&artifact, &generated);

        assert!(has_failure(&failures, REGISTRY_ARTIFACT));
        assert!(has_failure(&failures, "export_component_registry"));
    }

    #[test]
    fn registry_artifact_drift_reports_missing_generated_entry_by_name() {
        let artifact = minimal_manifest(&["Button"], "[]");
        let generated = minimal_manifest(&["Button", "TableColumnVisibility"], "[]");

        let failures = registry_artifact_failures_for_sources(&artifact, &generated);

        assert!(has_failure(
            &failures,
            "manifest entry `TableColumnVisibility`"
        ));
    }

    #[test]
    fn registry_artifact_drift_reports_missing_generated_recipe_by_id() {
        let artifact = minimal_manifest(&["Button"], "[]");
        let generated = minimal_manifest(
            &["Button"],
            r#"[{
                "id":"table-filters-toolbar",
                "source_components":["Button"],
                "generated_files":[{"path_hint":"src/ui/table_filters_toolbar.rs","intent":"compose"}],
                "verification_gates":["cargo test"]
            }]"#,
        );

        let failures = registry_artifact_failures_for_sources(&artifact, &generated);

        assert!(has_failure(
            &failures,
            "scaffold recipe `table-filters-toolbar`"
        ));
    }

    #[test]
    fn schema_artifact_drift_reports_path_and_regeneration_command() {
        let artifact = r#"{"type":"object","properties":{"schema_version":{}}}"#;
        let generated = r#"{"type":"object","properties":{"schema_version":{},"entries":{}}}"#;

        let failures = schema_artifact_failures_for_sources(artifact, generated);

        assert!(has_failure(&failures, SCHEMA_ARTIFACT));
        assert!(has_failure(&failures, "export_component_registry_schema"));
    }

    #[test]
    fn registry_manifest_shape_reports_missing_required_entry() {
        let manifest = minimal_manifest(&["Button"], "[]");

        let failures = registry_manifest_shape_failures(&manifest);

        assert!(has_failure(&failures, "Command"));
        assert!(has_failure(&failures, "TableFacetedFilter"));
    }

    #[test]
    fn recipe_reference_failures_name_recipe_and_missing_registry_row() {
        let recipes = r#"[{
            "id":"broken-recipe",
            "source_components":["MissingComponent"],
            "generated_files":[{"path_hint":"src/ui/broken.rs","intent":"broken"}],
            "verification_gates":["cargo test"]
        }]"#;
        let manifest = minimal_manifest(REQUIRED_REGISTRY_ENTRIES, recipes);

        let failures = registry_manifest_shape_failures(&manifest);

        assert!(has_failure(&failures, "broken-recipe"));
        assert!(has_failure(&failures, "MissingComponent"));
    }

    #[test]
    fn required_schema_marker_scan_reports_missing_current_vocabulary() {
        let failures = required_schema_marker_failures(
            r#"{"properties":{"schema_version":{},"entries":{}},"enum":["official_component"]}"#,
        );

        assert!(has_failure(&failures, "recipes"));
        assert!(has_failure(&failures, "app_owned_source"));
    }
}
