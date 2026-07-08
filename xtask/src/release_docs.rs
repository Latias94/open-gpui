use std::{
    fs,
    path::{Path, PathBuf},
};

use toml::Value;

const BREAKING_INVENTORY_PATH: &str = "docs/release/breaking-changes.md";

const REQUIRED_CRATE_READMES: &[RequiredCrateReadme] = &[
    RequiredCrateReadme {
        package: "open-gpui",
        manifest: "crates/gpui/Cargo.toml",
        readme: "crates/gpui/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-ui-components",
        manifest: "crates/ui_components/Cargo.toml",
        readme: "crates/ui_components/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-form",
        manifest: "crates/form/Cargo.toml",
        readme: "crates/form/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-resource",
        manifest: "crates/resource/Cargo.toml",
        readme: "crates/resource/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-devtools",
        manifest: "crates/devtools/Cargo.toml",
        readme: "crates/devtools/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-motion",
        manifest: "crates/motion/Cargo.toml",
        readme: "crates/motion/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-docking",
        manifest: "crates/gpui_docking/Cargo.toml",
        readme: "crates/gpui_docking/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-canvas",
        manifest: "crates/canvas/Cargo.toml",
        readme: "crates/canvas/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-web",
        manifest: "crates/gpui_web/Cargo.toml",
        readme: "crates/gpui_web/README.md",
    },
    RequiredCrateReadme {
        package: "open-gpui-platform",
        manifest: "crates/gpui_platform/Cargo.toml",
        readme: "crates/gpui_platform/README.md",
    },
];

#[derive(Debug, Clone, Copy)]
struct RequiredCrateReadme {
    package: &'static str,
    manifest: &'static str,
    readme: &'static str,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ReleaseDocsOptions {
    version: Option<String>,
    notes_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakingInventoryCoverage {
    Unreleased,
    SelectedRelease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BreakingChangeRow {
    crate_name: String,
    old_path: String,
    replacement: String,
    reason: String,
    release_note: String,
    verification: String,
}

pub(crate) fn verify_release_docs(root: &Path, args: &[String]) -> Result<(), ()> {
    let options = ReleaseDocsOptions::from_args(args)?;
    let version = match options.version.clone() {
        Some(version) => version,
        None => workspace_version(root)?,
    };

    println!("==> verify release docs for {version}");
    let breaking_inventory_coverage = options.breaking_inventory_coverage();
    let mut failures = release_doc_failures(root, &version, breaking_inventory_coverage);

    let changelog_path = root.join("CHANGELOG.md");
    let changelog = read_file(&changelog_path, "CHANGELOG.md", &mut failures);
    let release_notes = changelog
        .as_deref()
        .and_then(|source| changelog_section(source, &format!("[{version}]")))
        .map(str::trim)
        .map(str::to_string);

    if failures.is_empty() {
        if let Some(output) = options.notes_output {
            let output = if output.is_absolute() {
                output
            } else {
                root.join(output)
            };
            let Some(release_notes) = release_notes else {
                eprintln!("CHANGELOG.md: missing release notes for {version}");
                return Err(());
            };
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    eprintln!(
                        "{}: failed to create parent directory: {error}",
                        output.display()
                    );
                })?;
            }
            fs::write(&output, format!("{release_notes}\n")).map_err(|error| {
                eprintln!(
                    "{}: failed to write release notes: {error}",
                    output.display()
                );
            })?;
            println!("wrote release notes to {}", output.display());
        }

        println!("release docs verification passed");
        Ok(())
    } else {
        eprintln!("release docs verification failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn release_doc_failures(
    root: &Path,
    version: &str,
    breaking_inventory_coverage: BreakingInventoryCoverage,
) -> Vec<String> {
    let mut failures = Vec::new();
    let changelog_path = root.join("CHANGELOG.md");
    let Some(changelog) = read_file(&changelog_path, "CHANGELOG.md", &mut failures) else {
        return failures;
    };

    let selected_label = format!("[{version}]");
    let Some(selected_section) = changelog_section(&changelog, &selected_label) else {
        failures.push(format!(
            "CHANGELOG.md: missing release section `## {selected_label}`"
        ));
        return failures;
    };
    if selected_section.trim().is_empty() {
        failures.push(format!(
            "CHANGELOG.md: release section `## {selected_label}` is empty"
        ));
    }
    failures.extend(changelog_manual_wrap_failures(
        &format!("CHANGELOG.md {selected_label}"),
        selected_section,
    ));

    let unreleased = changelog_section(&changelog, "[Unreleased]");
    if let Some(unreleased) = unreleased {
        failures.extend(changelog_manual_wrap_failures(
            "CHANGELOG.md [Unreleased]",
            unreleased,
        ));
    } else {
        failures.push("CHANGELOG.md: missing `## [Unreleased]` section".to_string());
    }

    let (inventory_section_label, inventory_section) = match breaking_inventory_coverage {
        BreakingInventoryCoverage::SelectedRelease => (selected_label.as_str(), selected_section),
        BreakingInventoryCoverage::Unreleased => {
            if let Some(unreleased) = unreleased {
                ("[Unreleased]", unreleased)
            } else {
                ("[Unreleased]", "")
            }
        }
    };
    if !inventory_section.is_empty() {
        failures.extend(breaking_inventory_failures(
            root,
            inventory_section_label,
            inventory_section,
        ));
    }

    failures.extend(version_snippet_failures(root, version));
    failures.extend(crate_readme_failures(root));
    failures
}

fn workspace_version(root: &Path) -> Result<String, ()> {
    let manifest_path = root.join("Cargo.toml");
    let source = fs::read_to_string(&manifest_path).map_err(|error| {
        eprintln!("Cargo.toml: failed to read workspace manifest: {error}");
    })?;
    let manifest = toml::from_str::<Value>(&source).map_err(|error| {
        eprintln!("Cargo.toml: failed to parse workspace manifest: {error}");
    })?;
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            eprintln!("Cargo.toml: missing workspace.package.version");
        })
        .map_err(|_| ())
}

fn read_file(path: &Path, label: &str, failures: &mut Vec<String>) -> Option<String> {
    fs::read_to_string(path).map_or_else(
        |error| {
            failures.push(format!("{label}: failed to read file: {error}"));
            None
        },
        Some,
    )
}

fn changelog_section<'a>(contents: &'a str, label: &str) -> Option<&'a str> {
    let lines = contents.lines().collect::<Vec<_>>();
    let start_heading = lines
        .iter()
        .position(|line| line.starts_with("## ") && line.contains(label))?;
    let body_start = start_heading + 1;
    let body_end = lines[body_start..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |offset| body_start + offset);

    let start_byte: usize = lines[..body_start].iter().map(|line| line.len() + 1).sum();
    let end_byte: usize = lines[..body_end].iter().map(|line| line.len() + 1).sum();
    contents.get(start_byte..end_byte.min(contents.len()))
}

fn changelog_manual_wrap_failures(section_label: &str, section: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let mut in_fence = false;
    let mut previous_can_wrap = None;

    for (index, line) in section.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            previous_can_wrap = None;
            continue;
        }

        if in_fence || trimmed.is_empty() {
            previous_can_wrap = None;
            continue;
        }

        if is_markdown_boundary(trimmed) {
            previous_can_wrap = line_can_wrap(trimmed).then_some(line_number);
            continue;
        }

        if let Some(previous) = previous_can_wrap {
            failures.push(format!(
                "{section_label}: line {line_number} appears to manually wrap line {previous}; keep release-note paragraphs and bullets on one line"
            ));
        }
        previous_can_wrap = Some(line_number);
    }

    failures
}

fn is_markdown_boundary(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("> ")
        || line.starts_with('|')
        || line.starts_with("<!--")
        || ordered_list_start(line)
}

fn line_can_wrap(line: &str) -> bool {
    line.starts_with("- ") || line.starts_with("* ") || ordered_list_start(line)
}

fn ordered_list_start(line: &str) -> bool {
    let mut chars = line.chars().peekable();
    let mut saw_digit = false;
    while chars.peek().is_some_and(char::is_ascii_digit) {
        saw_digit = true;
        chars.next();
    }
    saw_digit && chars.next() == Some('.') && chars.next() == Some(' ')
}

fn version_snippet_failures(root: &Path, expected_version: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for path in version_checked_docs(root) {
        let label = display_path(root, &path);
        let Some(contents) = read_file(&path, &label, &mut failures) else {
            continue;
        };
        for (line_number, version) in version_snippets(&contents) {
            if version != expected_version {
                failures.push(format!(
                    "{label}:{line_number}: dependency snippet uses version `{version}` but release docs expect `{expected_version}`"
                ));
            }
        }
    }
    failures
}

fn version_checked_docs(root: &Path) -> Vec<PathBuf> {
    let mut docs = vec![root.join("README.md")];
    docs.extend(
        REQUIRED_CRATE_READMES
            .iter()
            .map(|entry| root.join(entry.readme)),
    );
    docs
}

fn version_snippets(contents: &str) -> Vec<(usize, String)> {
    let mut snippets = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let mut rest = line;
        while let Some(start) = rest.find("version = \"") {
            rest = &rest[start + "version = \"".len()..];
            let Some(end) = rest.find('"') else {
                break;
            };
            let version = &rest[..end];
            if looks_like_semver(version) {
                snippets.push((index + 1, version.to_string()));
            }
            rest = &rest[end + 1..];
        }
    }
    snippets
}

fn looks_like_semver(version: &str) -> bool {
    let parts = version
        .split_once('-')
        .map_or(version, |(core, _)| core)
        .split('.')
        .collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn crate_readme_failures(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    for entry in REQUIRED_CRATE_READMES {
        let manifest_path = root.join(entry.manifest);
        let Some(source) = read_file(&manifest_path, entry.manifest, &mut failures) else {
            continue;
        };
        let manifest = match toml::from_str::<Value>(&source) {
            Ok(manifest) => manifest,
            Err(error) => {
                failures.push(format!(
                    "{}: failed to parse manifest: {error}",
                    entry.manifest
                ));
                continue;
            }
        };
        let package = manifest.get("package").unwrap_or(&Value::Boolean(false));
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if name != entry.package {
            failures.push(format!(
                "{}: expected package `{}`, found `{name}`",
                entry.manifest, entry.package
            ));
        }

        match package.get("readme") {
            Some(Value::String(readme)) if readme == "README.md" => {}
            Some(Value::Table(table))
                if table.get("workspace").and_then(Value::as_bool) == Some(true) =>
            {
                failures.push(format!(
                    "{}: `{}` must use crate-local `readme = \"README.md\"`, not `readme.workspace = true`",
                    entry.manifest, entry.package
                ));
            }
            Some(other) => failures.push(format!(
                "{}: `{}` must use crate-local `readme = \"README.md\"`, found `{other}`",
                entry.manifest, entry.package
            )),
            None => failures.push(format!(
                "{}: `{}` is missing crate-local `readme = \"README.md\"`",
                entry.manifest, entry.package
            )),
        }

        if !root.join(entry.readme).is_file() {
            failures.push(format!(
                "{}: required README for `{}` does not exist",
                entry.readme, entry.package
            ));
        }
    }
    failures
}

fn breaking_inventory_failures(
    root: &Path,
    changelog_section_label: &str,
    changelog_section: &str,
) -> Vec<String> {
    let mut failures = Vec::new();
    let path = root.join(BREAKING_INVENTORY_PATH);
    let Some(contents) = read_file(&path, BREAKING_INVENTORY_PATH, &mut failures) else {
        return failures;
    };
    let rows = breaking_change_rows(&contents, &mut failures);
    if rows.is_empty() {
        return failures;
    }
    let changelog_failure_prefix = format!("CHANGELOG.md {changelog_section_label}");
    if !changelog_section.contains("### Breaking Changes and Migration Notes") {
        failures.push(format!(
            "{changelog_failure_prefix}: breaking inventory has rows but the changelog has no breaking-change section"
        ));
    }

    for row in rows {
        for (field, value) in [
            ("Crate", &row.crate_name),
            ("Old path", &row.old_path),
            ("New path or replacement", &row.replacement),
            ("Reason", &row.reason),
            ("Release-note text", &row.release_note),
            ("Verification", &row.verification),
        ] {
            if value.trim().is_empty() || value.contains("TBD") || value.contains("TODO") {
                failures.push(format!(
                    "{BREAKING_INVENTORY_PATH}: row for `{}` has incomplete `{field}`",
                    row.old_path
                ));
            }
        }

        if let Some(symbol) = old_path_symbol(&row.old_path) {
            if !changelog_section.contains(&symbol) {
                failures.push(format!(
                    "{changelog_failure_prefix}: breaking-change inventory symbol `{symbol}` is missing from release-facing notes"
                ));
            }
        }
    }

    failures
}

fn breaking_change_rows(contents: &str, failures: &mut Vec<String>) -> Vec<BreakingChangeRow> {
    let Some(section) = markdown_section(contents, "## Next Release") else {
        failures.push(format!(
            "{BREAKING_INVENTORY_PATH}: missing `## Next Release` section"
        ));
        return Vec::new();
    };

    let mut rows = Vec::new();
    for line in section.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || trimmed.contains("|---") || trimmed.starts_with("| Crate |")
        {
            continue;
        }
        let cells = markdown_table_cells(trimmed);
        if cells.len() != 6 {
            failures.push(format!(
                "{BREAKING_INVENTORY_PATH}: malformed breaking-change row `{trimmed}`"
            ));
            continue;
        }
        rows.push(BreakingChangeRow {
            crate_name: cells[0].clone(),
            old_path: cells[1].clone(),
            replacement: cells[2].clone(),
            reason: cells[3].clone(),
            release_note: cells[4].clone(),
            verification: cells[5].clone(),
        });
    }
    rows
}

fn markdown_section<'a>(contents: &'a str, heading: &str) -> Option<&'a str> {
    let lines = contents.lines().collect::<Vec<_>>();
    let start_heading = lines.iter().position(|line| line.trim() == heading)?;
    let body_start = start_heading + 1;
    let body_end = lines[body_start..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |offset| body_start + offset);
    let start_byte: usize = lines[..body_start].iter().map(|line| line.len() + 1).sum();
    let end_byte: usize = lines[..body_end].iter().map(|line| line.len() + 1).sum();
    contents.get(start_byte..end_byte.min(contents.len()))
}

fn markdown_table_cells(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn old_path_symbol(old_path: &str) -> Option<String> {
    let code = first_code_span(old_path)?;
    let symbol = if code.starts_with("open_") && code.contains("::") {
        code.rsplit("::").next().unwrap_or(code).to_string()
    } else if code.contains("::") {
        code.split_once('(')
            .map_or(code, |(symbol, _)| symbol)
            .to_string()
    } else {
        code.split_once('(')
            .map_or(code, |(symbol, _)| symbol)
            .to_string()
    };
    let symbol = symbol.trim_matches('`').trim().to_string();
    (!symbol.is_empty()).then_some(symbol)
}

fn first_code_span(value: &str) -> Option<&str> {
    let start = value.find('`')? + 1;
    let end = value[start..].find('`')? + start;
    value.get(start..end)
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

impl ReleaseDocsOptions {
    fn breaking_inventory_coverage(&self) -> BreakingInventoryCoverage {
        if self.version.is_some() || self.notes_output.is_some() {
            BreakingInventoryCoverage::SelectedRelease
        } else {
            BreakingInventoryCoverage::Unreleased
        }
    }

    fn from_args(args: &[String]) -> Result<Self, ()> {
        let mut options = Self::default();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--version" => {
                    index += 1;
                    let Some(version) = args.get(index) else {
                        eprintln!("--version requires a value");
                        return Err(());
                    };
                    options.version = Some(version.clone());
                }
                "--notes-output" => {
                    index += 1;
                    let Some(path) = args.get(index) else {
                        eprintln!("--notes-output requires a path");
                        return Err(());
                    };
                    options.notes_output = Some(PathBuf::from(path));
                }
                unknown => {
                    eprintln!("unknown verify-release-docs argument: {unknown}");
                    return Err(());
                }
            }
            index += 1;
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_section_extracts_only_requested_version() {
        let changelog = "\
# Changelog

## [Unreleased]

- Next

## [0.2.0] - 2026-07-07

- Current

## [0.1.0] - 2026-06-09

- Old
";
        assert_eq!(
            changelog_section(changelog, "[0.2.0]").unwrap().trim(),
            "- Current"
        );
    }

    #[test]
    fn changelog_wrap_scan_rejects_continuation_lines() {
        let failures = changelog_manual_wrap_failures(
            "CHANGELOG.md [1.0.0]",
            "\
### Changed

- First half
  second half
",
        );
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn version_snippet_scan_finds_toml_dependency_versions() {
        assert_eq!(
            version_snippets(r#"open_gpui = { package = "open-gpui", version = "0.2.0" }"#),
            vec![(1, "0.2.0".to_string())]
        );
    }

    #[test]
    fn explicit_release_options_check_breaking_inventory_against_selected_release() {
        assert_eq!(
            ReleaseDocsOptions::default().breaking_inventory_coverage(),
            BreakingInventoryCoverage::Unreleased
        );
        assert_eq!(
            ReleaseDocsOptions {
                version: Some("0.3.0".to_string()),
                notes_output: None,
            }
            .breaking_inventory_coverage(),
            BreakingInventoryCoverage::SelectedRelease
        );
        assert_eq!(
            ReleaseDocsOptions {
                version: None,
                notes_output: Some(PathBuf::from("release-notes.md")),
            }
            .breaking_inventory_coverage(),
            BreakingInventoryCoverage::SelectedRelease
        );
    }

    #[test]
    fn breaking_inventory_parser_reads_rows() {
        let mut failures = Vec::new();
        let rows = breaking_change_rows(
            "\
# Breaking Change Inventory

## Next Release

| Crate | Old path | New path or replacement | Reason | Release-note text | Verification |
|---|---|---|---|---|---|
| `open-gpui` | `open_gpui::Old` | `open_gpui::New` | Better boundary. | `open-gpui` moved `Old`. | `cargo test` |
",
            &mut failures,
        );
        assert!(failures.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].old_path, "`open_gpui::Old`");
    }

    #[test]
    fn old_path_symbol_prefers_terminal_public_path_segment() {
        assert_eq!(
            old_path_symbol("`open_gpui_docking::DockTransitionPlan`"),
            Some("DockTransitionPlan".to_string())
        );
        assert_eq!(
            old_path_symbol("`MotionFrameHost::reset()`"),
            Some("MotionFrameHost::reset".to_string())
        );
    }
}
