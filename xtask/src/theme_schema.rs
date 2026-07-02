use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;

const SCHEMA_ARTIFACT: &str = "docs/schemas/open-gpui-theme-v1.schema.json";
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

    let generated_source = match generated_theme_schema(root) {
        Ok(source) => source,
        Err(error) => return vec![error],
    };

    theme_schema_failures_for_sources(&artifact_source, &generated_source)
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

fn normalized_json(source: &str, label: &str, failures: &mut Vec<String>) -> Option<String> {
    let value = match serde_json::from_str::<Value>(source) {
        Ok(value) => value,
        Err(error) => {
            failures.push(format!("{label}: failed to parse JSON: {error}"));
            return None;
        }
    };

    serde_json::to_string_pretty(&value)
        .map(|normalized| format!("{normalized}\n"))
        .map_err(|error| {
            failures.push(format!("{label}: failed to normalize JSON: {error}"));
        })
        .ok()
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
}
