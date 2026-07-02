use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;

const SCHEMA_ARTIFACT: &str = "docs/schemas/open-gpui-theme-v1.schema.json";
const COMPONENT_CONTRACT_DOCS: &str = "docs/ui/component-contract.md";
const DOCS_SCHEMA_VOCABULARY_MARKER: &str = "Schema vocabulary audit target:";
const EXPORT_COMMAND: &[&str] = &[
    "run",
    "-p",
    "open-gpui-ui-components",
    "--example",
    "export_theme_schema",
    "--quiet",
];

const REQUIRED_SCHEMA_MARKERS: &[&str] = &[
    "schema_version",
    "id",
    "label",
    "mode",
    "revision",
    "fallback_mode",
    "colors",
    "token",
    "state",
    "rgb",
    "semantic.surface",
    "semantic.surface_muted",
    "semantic.focus_ring",
    "semantic.modal_overlay",
    "default",
    "hover",
    "selected",
    "disabled",
    "read-only",
    "invalid",
    "required",
    "placeholder",
    "message",
    "focus-visible",
    "overlay",
    "modal-overlay",
    "light",
    "dark",
    "high-contrast",
];

pub(crate) fn scan_theme_schema(root: &Path) -> Result<(), ()> {
    println!("==> scan theme schema");

    let failures = theme_schema_failures(root);
    if failures.is_empty() {
        println!("theme schema scan passed");
        Ok(())
    } else {
        eprintln!("theme schema scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

pub(crate) fn theme_schema_failures(root: &Path) -> Vec<String> {
    let artifact_path = root.join(SCHEMA_ARTIFACT);
    let artifact_source = match fs::read_to_string(&artifact_path) {
        Ok(source) => source,
        Err(error) => {
            return vec![format!(
                "{SCHEMA_ARTIFACT}: failed to read theme schema artifact: {error}"
            )];
        }
    };

    let mut failures = match generated_theme_schema(root) {
        Ok(generated_source) => {
            theme_schema_failures_for_sources(&artifact_source, &generated_source)
        }
        Err(error) => vec![error],
    };

    let docs_path = root.join(COMPONENT_CONTRACT_DOCS);
    match fs::read_to_string(&docs_path) {
        Ok(docs_source) => {
            failures.extend(theme_schema_docs_failures(&artifact_source, &docs_source))
        }
        Err(error) => failures.push(format!(
            "{COMPONENT_CONTRACT_DOCS}: failed to read theme schema docs vocabulary: {error}"
        )),
    }

    failures
}

fn generated_theme_schema(root: &Path) -> Result<String, String> {
    let output = Command::new("cargo")
        .args(EXPORT_COMMAND)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("cargo {}: failed to run: {error}", EXPORT_COMMAND.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cargo {}: schema export failed: {}",
            EXPORT_COMMAND.join(" "),
            stderr.trim()
        ));
    }

    String::from_utf8(output.stdout).map_err(|error| {
        format!(
            "cargo {}: emitted invalid UTF-8: {error}",
            EXPORT_COMMAND.join(" ")
        )
    })
}

fn theme_schema_failures_for_sources(artifact_source: &str, generated_source: &str) -> Vec<String> {
    let mut failures = Vec::new();
    if let (Some(artifact), Some(generated)) = (
        normalized_json(artifact_source, SCHEMA_ARTIFACT, &mut failures),
        normalized_json(generated_source, "theme_json_schema()", &mut failures),
    ) {
        if artifact != generated {
            failures.push(format!(
                "{SCHEMA_ARTIFACT}: theme_json_schema() drifted from the committed artifact; regenerate with `cargo run -p open-gpui-ui-components --example export_theme_schema --quiet` and replace this file"
            ));
        }
    }

    failures.extend(required_schema_marker_failures(artifact_source));
    failures
}

fn theme_schema_docs_failures(artifact_source: &str, docs_source: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let Some(schema) = parse_json_value(artifact_source, SCHEMA_ARTIFACT, &mut failures) else {
        return failures;
    };
    let vocabulary = schema_vocabulary(&schema, &mut failures);
    let Some(block) = docs_schema_vocabulary_block(docs_source) else {
        failures.push(format!(
            "{COMPONENT_CONTRACT_DOCS}: missing `{DOCS_SCHEMA_VOCABULARY_MARKER}` block for theme schema docs drift audit"
        ));
        return failures;
    };

    let documented = markdown_code_literals(block);
    for (label, values) in vocabulary.groups() {
        for value in values {
            if !documented.contains(value) {
                failures.push(format!(
                    "{COMPONENT_CONTRACT_DOCS}: theme schema vocabulary block is missing {label} `{value}`"
                ));
            }
        }
    }

    let allowed = vocabulary.all_values();
    for value in documented {
        if !allowed.contains(&value) {
            failures.push(format!(
                "{COMPONENT_CONTRACT_DOCS}: theme schema vocabulary block lists unsupported value `{value}`; add it to theme_json_schema() before documenting it"
            ));
        }
    }

    failures
}

struct SchemaVocabulary {
    top_level_fields: BTreeSet<String>,
    color_fields: BTreeSet<String>,
    modes: BTreeSet<String>,
    tokens: BTreeSet<String>,
    states: BTreeSet<String>,
}

impl SchemaVocabulary {
    fn groups(&self) -> [(&'static str, &BTreeSet<String>); 5] {
        [
            ("top-level field", &self.top_level_fields),
            ("color entry field", &self.color_fields),
            ("mode", &self.modes),
            ("token", &self.tokens),
            ("state", &self.states),
        ]
    }

    fn all_values(&self) -> BTreeSet<String> {
        self.groups()
            .into_iter()
            .flat_map(|(_, values)| values.iter().cloned())
            .collect()
    }
}

fn schema_vocabulary(schema: &Value, failures: &mut Vec<String>) -> SchemaVocabulary {
    SchemaVocabulary {
        top_level_fields: object_property_keys(schema, "/properties", "top-level fields", failures),
        color_fields: object_property_keys(
            schema,
            "/$defs/ThemeJsonColorEntry/properties",
            "color entry fields",
            failures,
        ),
        modes: string_enum_values(schema, "/$defs/ThemeJsonMode/enum", "theme modes", failures),
        tokens: string_enum_values(
            schema,
            "/$defs/ThemeJsonToken/enum",
            "theme tokens",
            failures,
        ),
        states: string_enum_values(
            schema,
            "/$defs/ThemeJsonColorState/enum",
            "theme states",
            failures,
        ),
    }
}

fn object_property_keys(
    schema: &Value,
    pointer: &str,
    label: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(object) = schema.pointer(pointer).and_then(Value::as_object) else {
        failures.push(format!("{SCHEMA_ARTIFACT}: missing {label} at `{pointer}`"));
        return BTreeSet::new();
    };
    object.keys().cloned().collect()
}

fn string_enum_values(
    schema: &Value,
    pointer: &str,
    label: &str,
    failures: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(values) = schema.pointer(pointer).and_then(Value::as_array) else {
        failures.push(format!(
            "{SCHEMA_ARTIFACT}: missing {label} enum at `{pointer}`"
        ));
        return BTreeSet::new();
    };

    let mut strings = BTreeSet::new();
    for value in values {
        match value.as_str() {
            Some(value) => {
                strings.insert(value.to_string());
            }
            None => failures.push(format!(
                "{SCHEMA_ARTIFACT}: {label} enum contains non-string value `{value}`"
            )),
        }
    }
    strings
}

fn docs_schema_vocabulary_block(source: &str) -> Option<&str> {
    let start = source.find(DOCS_SCHEMA_VOCABULARY_MARKER)?;
    let tail = &source[start + DOCS_SCHEMA_VOCABULARY_MARKER.len()..];
    let end = tail
        .find("\r\n\r\nTheme module ownership")
        .or_else(|| tail.find("\n\nTheme module ownership"))
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

fn markdown_code_literals(source: &str) -> BTreeSet<String> {
    let mut literals = BTreeSet::new();
    let mut rest = source;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('`') else {
            break;
        };
        let literal = rest[..end].trim();
        if !literal.is_empty() {
            literals.insert(literal.to_string());
        }
        rest = &rest[end + 1..];
    }
    literals
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

fn required_schema_marker_failures(artifact_source: &str) -> Vec<String> {
    REQUIRED_SCHEMA_MARKERS
        .iter()
        .filter(|marker| !artifact_source.contains(**marker))
        .map(|marker| {
            format!("{SCHEMA_ARTIFACT}: missing current theme JSON schema marker `{marker}`")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_failure(failures: &[String], needle: &str) -> bool {
        failures.iter().any(|failure| failure.contains(needle))
    }

    fn minimal_schema() -> &'static str {
        r##"{
  "type": "object",
  "properties": {
    "schema_version": {},
    "colors": {}
  },
  "$defs": {
    "ThemeJsonColorEntry": {
      "properties": {
        "token": {},
        "state": {},
        "rgb": {}
      }
    },
    "ThemeJsonMode": {
      "enum": ["light"]
    },
    "ThemeJsonToken": {
      "enum": ["semantic.surface"]
    },
    "ThemeJsonColorState": {
      "enum": ["default"]
    }
  }
}"##
    }

    fn docs_with_vocabulary(values: &[&str]) -> String {
        let values = values
            .iter()
            .map(|value| format!("`{value}`"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{DOCS_SCHEMA_VOCABULARY_MARKER}\n\n- Values: {values}\n\nTheme module ownership is intentionally split:"
        )
    }

    #[test]
    fn docs_vocabulary_block_handles_crlf_boundaries() {
        let source = docs_with_vocabulary(&["schema_version", "colors"]).replace('\n', "\r\n");

        let block =
            docs_schema_vocabulary_block(&source).expect("docs vocabulary block should be found");

        assert!(block.contains("schema_version"));
        assert!(!block.contains("Theme module ownership"));
    }

    #[test]
    fn schema_drift_reports_artifact_mismatch() {
        let artifact = r#"{"type":"object","properties":{"schema_version":{"type":"integer"}}}"#;
        let generated = r#"{"type":"object","properties":{"schema_version":{"type":"integer"},"fallback_mode":{"type":"string"}}}"#;

        let failures = theme_schema_failures_for_sources(artifact, generated);

        assert!(has_failure(&failures, "theme_json_schema() drifted"));
    }

    #[test]
    fn required_marker_scan_reports_missing_current_vocabulary() {
        let failures = required_schema_marker_failures(
            r#"{"enum":["schema_version","fallback_mode","semantic.focus_ring"]}"#,
        );

        assert!(has_failure(&failures, "semantic.modal_overlay"));
        assert!(has_failure(&failures, "focus-visible"));
        assert!(has_failure(&failures, "high-contrast"));
    }

    #[test]
    fn docs_vocabulary_reports_unsupported_schema_values() {
        let docs = docs_with_vocabulary(&[
            "schema_version",
            "colors",
            "token",
            "state",
            "rgb",
            "light",
            "semantic.surface",
            "default",
            "pressed",
        ]);

        let failures = theme_schema_docs_failures(minimal_schema(), &docs);

        assert!(has_failure(&failures, "unsupported value `pressed`"));
    }

    #[test]
    fn docs_vocabulary_reports_missing_schema_values() {
        let docs = docs_with_vocabulary(&[
            "schema_version",
            "colors",
            "token",
            "state",
            "rgb",
            "light",
            "default",
        ]);

        let failures = theme_schema_docs_failures(minimal_schema(), &docs);

        assert!(has_failure(&failures, "token `semantic.surface`"));
    }
}
