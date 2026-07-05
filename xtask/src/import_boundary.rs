use std::{fs, path::Path};

use crate::fs_scan::{display_path, tracked_text_files};

const DISALLOWED_DEPENDENCY_NAMES: &[&str] = &[
    "ztracing",
    "ztracing_macro",
    "zlog",
    "zed-sum-tree",
    "zed-scap",
    "zed-font-kit",
    "perf",
];
const DISALLOWED_ZED_GIT_SOURCES: &[&str] = &[
    "zed-industries/font-kit",
    "zed-industries/reqwest",
    "zed-industries/scap",
    "zed-industries/wgpu",
    "zed-industries/xim",
];

pub(crate) fn scan_import_boundary(root: &Path) -> Result<(), ()> {
    println!("==> scan import boundary");

    let mut failures = Vec::new();
    scan_dependency_files(root, &mut failures);
    scan_license_files(root, &mut failures);

    if failures.is_empty() {
        println!("import boundary scan passed");
        Ok(())
    } else {
        eprintln!("import boundary scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn scan_dependency_files(root: &Path, failures: &mut Vec<String>) {
    for path in tracked_text_files(root, is_dependency_file) {
        let relative = display_path(root, &path);
        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{relative}: failed to read file"));
            continue;
        };

        match path.file_name().and_then(|name| name.to_str()) {
            Some("Cargo.toml") => scan_manifest_dependencies(&relative, &contents, failures),
            Some("Cargo.lock") => scan_lock_dependencies(&relative, &contents, failures),
            _ => {}
        }
    }
}

fn scan_manifest_dependencies(relative: &str, contents: &str, failures: &mut Vec<String>) {
    let document = match toml::from_str::<toml::Value>(contents) {
        Ok(document) => document,
        Err(error) => {
            failures.push(format!("{relative}: failed to parse TOML: {error}"));
            return;
        }
    };

    let Some(table) = document.as_table() else {
        failures.push(format!("{relative}: expected TOML document table"));
        return;
    };

    scan_manifest_table(relative, table, &mut Vec::new(), failures);
}

fn scan_manifest_table(
    relative: &str,
    table: &toml::map::Map<String, toml::Value>,
    section: &mut Vec<String>,
    failures: &mut Vec<String>,
) {
    for (key, value) in table {
        let Some(child) = value.as_table() else {
            continue;
        };

        section.push(key.clone());
        if is_manifest_dependency_collection(section) {
            scan_manifest_dependency_table(relative, section, child, failures);
        } else {
            scan_manifest_table(relative, child, section, failures);
        }
        section.pop();
    }
}

fn is_manifest_dependency_collection(section: &[String]) -> bool {
    let Some(last) = section.last().map(String::as_str) else {
        return false;
    };

    let cargo_dependency_section = matches!(
        last,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    ) && (section.len() == 1
        || (section.len() == 2 && section[0] == "workspace")
        || section[0] == "target");

    cargo_dependency_section
        || (section.len() >= 2 && section[0] == "patch")
        || (section.len() == 1 && section[0] == "replace")
}

fn scan_manifest_dependency_table(
    relative: &str,
    section: &[String],
    dependencies: &toml::map::Map<String, toml::Value>,
    failures: &mut Vec<String>,
) {
    let section = section.join(".");

    for (alias, spec) in dependencies {
        let alias_name = dependency_key_package_name(alias);
        if is_disallowed_dependency_name(alias_name) {
            failures.push(format!(
                "{relative}:{section}.{alias}: disallowed dependency name `{alias_name}`"
            ));
        }

        if let Some(package_name) = dependency_package_field(spec) {
            if package_name != alias_name && is_disallowed_dependency_name(package_name) {
                failures.push(format!(
                    "{relative}:{section}.{alias}: disallowed dependency package `{package_name}`"
                ));
            }
        }

        if let Some(git_source) = dependency_git_source(spec) {
            if is_zed_monorepo_source(git_source) {
                failures.push(format!(
                    "{relative}:{section}.{alias}: disallowed Zed monorepo git source `{git_source}`"
                ));
            } else if is_disallowed_zed_git_source(git_source) {
                failures.push(format!(
                    "{relative}:{section}.{alias}: disallowed retired Zed fork git source `{git_source}`"
                ));
            }
        }
    }
}

fn scan_lock_dependencies(relative: &str, contents: &str, failures: &mut Vec<String>) {
    let document = match toml::from_str::<toml::Value>(contents) {
        Ok(document) => document,
        Err(error) => {
            failures.push(format!("{relative}: failed to parse TOML: {error}"));
            return;
        }
    };

    let Some(packages) = document.get("package").and_then(toml::Value::as_array) else {
        return;
    };

    for package in packages {
        let Some(package) = package.as_table() else {
            continue;
        };

        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("<unknown>");
        if is_disallowed_dependency_name(name) {
            failures.push(format!(
                "{relative}: disallowed locked package name `{name}`"
            ));
        }

        if let Some(source) = package.get("source").and_then(toml::Value::as_str) {
            if is_zed_monorepo_source(source) {
                failures.push(format!(
                    "{relative}: package `{name}` uses disallowed Zed monorepo source `{source}`"
                ));
            } else if is_disallowed_zed_git_source(source) {
                failures.push(format!(
                    "{relative}: package `{name}` uses disallowed retired Zed fork source `{source}`"
                ));
            }
        }
    }
}

fn dependency_key_package_name(key: &str) -> &str {
    key.split_once(':')
        .map_or(key, |(package_name, _version)| package_name)
}

fn dependency_package_field(spec: &toml::Value) -> Option<&str> {
    spec.as_table()?
        .get("package")
        .and_then(toml::Value::as_str)
}

fn dependency_git_source(spec: &toml::Value) -> Option<&str> {
    spec.as_table()?.get("git").and_then(toml::Value::as_str)
}

fn is_disallowed_dependency_name(name: &str) -> bool {
    DISALLOWED_DEPENDENCY_NAMES.contains(&name)
}

fn is_zed_monorepo_source(source: &str) -> bool {
    normalized_github_path(source)
        .as_deref()
        .is_some_and(|path| path == "zed-industries/zed")
}

fn is_disallowed_zed_git_source(source: &str) -> bool {
    normalized_github_path(source)
        .as_deref()
        .is_some_and(|path| DISALLOWED_ZED_GIT_SOURCES.contains(&path))
}

fn normalized_github_path(source: &str) -> Option<String> {
    let source = source
        .trim()
        .strip_prefix("git+")
        .unwrap_or(source.trim())
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/');

    let path = source
        .strip_prefix("git@github.com:")
        .or_else(|| source.split_once("github.com/").map(|(_prefix, path)| path))?;

    Some(
        path.trim_end_matches('/')
            .trim_end_matches(".git")
            .to_ascii_lowercase(),
    )
}

fn scan_license_files(root: &Path, failures: &mut Vec<String>) {
    for path in tracked_text_files(root, is_license_or_manifest_file) {
        let relative = display_path(root, &path);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        if file_name.contains("LICENSE-GPL") {
            failures.push(format!(
                "{relative}: GPL license file is not allowed in this workspace"
            ));
            continue;
        }

        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{relative}: failed to read file"));
            continue;
        };

        for (index, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.contains("GNU GENERAL PUBLIC LICENSE") || trimmed.contains("GPL-3") {
                failures.push(format!(
                    "{relative}:{}: GPL marker is not allowed in manifests or license files",
                    index + 1
                ));
            }
        }
    }
}

fn is_dependency_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "Cargo.toml" | "Cargo.lock"))
}

fn is_license_or_manifest_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    matches!(name, "Cargo.toml" | "Cargo.lock") || name.starts_with("LICENSE")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_failures(contents: &str) -> Vec<String> {
        let mut failures = Vec::new();
        scan_manifest_dependencies("Cargo.toml", contents, &mut failures);
        failures
    }

    fn lock_failures(contents: &str) -> Vec<String> {
        let mut failures = Vec::new();
        scan_lock_dependencies("Cargo.lock", contents, &mut failures);
        failures
    }

    fn has_failure(failures: &[String], needle: &str) -> bool {
        failures.iter().any(|failure| failure.contains(needle))
    }

    #[test]
    fn manifest_scan_detects_alias_package_names() {
        let failures = manifest_failures(
            r#"
[dependencies]
trace = { package = 'ztracing', version = '0.1' }
"#,
        );

        assert!(has_failure(&failures, "ztracing"));
    }

    #[test]
    fn manifest_scan_detects_zed_git_url_variants() {
        let failures = manifest_failures(
            r#"
[dependencies]
zed={git='https://github.com/zed-industries/zed.git'}
"#,
        );

        assert!(has_failure(&failures, "zed-industries/zed.git"));
    }

    #[test]
    fn manifest_scan_detects_workspace_and_target_dependency_sections() {
        let failures = manifest_failures(
            r#"
[workspace.dependencies]
perf.workspace = true

[target.'cfg(windows)'.dependencies]
sum_tree = { package = 'zed-sum-tree', version = '0.1' }
font-kit = { package = 'zed-font-kit', version = '0.14.1-zed' }
"#,
        );

        assert!(has_failure(&failures, "perf"));
        assert!(has_failure(&failures, "zed-sum-tree"));
        assert!(has_failure(&failures, "zed-font-kit"));
    }

    #[test]
    fn manifest_scan_detects_patch_and_replace_sections() {
        let failures = manifest_failures(
            r#"
[patch.crates-io]
zlog = { version = '0.1' }

[replace]
'perf:0.1.0' = { git = 'https://github.com/example/perf' }
"#,
        );

        assert!(has_failure(&failures, "zlog"));
        assert!(has_failure(&failures, "perf"));
    }

    #[test]
    fn manifest_scan_rejects_retired_zed_git_forks() {
        let failures = manifest_failures(
            r#"
[dependencies]
reqwest = { git = 'https://github.com/zed-industries/reqwest.git', package = 'zed-reqwest', version = '0.12.15-zed' }
wgpu = { git = 'https://github.com/zed-industries/wgpu.git' }
font-kit = { git = 'https://github.com/zed-industries/font-kit.git', package = 'zed-font-kit' }
xim = { git = 'https://github.com/zed-industries/xim.git', package = 'zed-xim' }
"#,
        );

        assert!(has_failure(&failures, "zed-industries/reqwest.git"));
        assert!(has_failure(&failures, "zed-industries/wgpu.git"));
        assert!(has_failure(&failures, "zed-industries/font-kit.git"));
        assert!(has_failure(&failures, "zed-industries/xim.git"));
    }

    #[test]
    fn manifest_scan_rejects_retired_zed_scap_fork() {
        let failures = manifest_failures(
            r#"
[dependencies]
scap = { git = 'https://github.com/zed-industries/scap.git', package = 'zed-scap', version = '0.0.8-zed' }
"#,
        );

        assert!(has_failure(&failures, "zed-scap"));
        assert!(has_failure(&failures, "zed-industries/scap.git"));
    }

    #[test]
    fn manifest_scan_allows_open_gpui_scap_fork() {
        let failures = manifest_failures(
            r#"
[dependencies]
scap = { git = 'https://github.com/Latias94/scap', branch = 'main', package = 'open-gpui-scap', version = '0.1.0-beta.1' }
"#,
        );

        assert!(
            failures.is_empty(),
            "expected Open GPUI scap fork to be allowed, got: {failures:?}"
        );
    }

    #[test]
    fn lock_scan_detects_disallowed_package_names_and_sources() {
        let failures = lock_failures(
            r#"
[[package]]
name = "perf"
version = "0.1.0"

[[package]]
name = "zed-font-kit"
version = "0.14.1-zed"

[[package]]
name = "zed-scap"
version = "0.0.8-zed"

[[package]]
name = "allowed"
version = "0.1.0"
source = "git+https://github.com/zed-industries/zed.git?rev=abc#123"
"#,
        );

        assert!(has_failure(&failures, "perf"));
        assert!(has_failure(&failures, "zed-font-kit"));
        assert!(has_failure(&failures, "zed-scap"));
        assert!(has_failure(&failures, "zed-industries/zed.git"));
    }

    #[test]
    fn lock_scan_rejects_retired_zed_git_forks() {
        let failures = lock_failures(
            r#"
[[package]]
name = "wgpu"
version = "29.0.3"
source = "git+https://github.com/zed-industries/wgpu.git?rev=abc#123"
"#,
        );

        assert!(has_failure(&failures, "zed-industries/wgpu.git"));
    }
}
