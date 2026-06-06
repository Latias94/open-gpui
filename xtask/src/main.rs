use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

const DISALLOWED_DEPENDENCY_NAMES: &[&str] =
    &["ztracing", "ztracing_macro", "zlog", "zed-sum-tree", "perf"];

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::FAILURE;
    };

    let root = workspace_root();
    let result = match command.as_str() {
        "verify" => verify(&root),
        "scan-import-boundary" => scan_import_boundary(&root),
        _ => {
            eprintln!("unknown command: {command}");
            print_usage();
            Err(())
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn print_usage() {
    eprintln!("usage: cargo run -p xtask -- <command>");
    eprintln!();
    eprintln!("commands:");
    eprintln!("  verify                run the local Open GPUI gate");
    eprintln!("  scan-import-boundary  scan for disallowed import residue");
}

fn verify(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["fmt", "--all", "--check"])?;
    run(root, "cargo", &["check", "--workspace"])?;
    run(root, "cargo", &["check", "-p", "open-gpui-smoke-native"])?;
    scan_import_boundary(root)?;
    Ok(())
}

fn run(root: &Path, program: &str, args: &[&str]) -> Result<(), ()> {
    let display = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");

    println!("==> {display}");
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| {
            eprintln!("failed to run `{display}`: {error}");
        })?;

    if status.success() {
        Ok(())
    } else {
        eprintln!("command failed: {display}");
        Err(())
    }
}

fn scan_import_boundary(root: &Path) -> Result<(), ()> {
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
    let document = match contents.parse::<toml::Value>() {
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
            }
        }
    }
}

fn scan_lock_dependencies(relative: &str, contents: &str, failures: &mut Vec<String>) {
    let document = match contents.parse::<toml::Value>() {
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

fn tracked_text_files(root: &Path, include: fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, root, include, &mut files);
    files
}

fn collect_files(
    root: &Path,
    directory: &Path,
    include: fn(&Path) -> bool,
    files: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if path.is_dir() {
            if should_skip_dir(root, &path, &file_name) {
                continue;
            }
            collect_files(root, &path, include, files);
        } else if include(&path) {
            files.push(path);
        }
    }
}

fn should_skip_dir(root: &Path, path: &Path, file_name: &str) -> bool {
    if matches!(file_name, ".git" | "target" | "repo-ref") {
        return true;
    }

    path.strip_prefix(root).ok().is_some_and(|relative| {
        relative
            .components()
            .any(|component| component.as_os_str() == "target")
    })
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

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest should live under the workspace root")
        .to_path_buf()
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
"#,
        );

        assert!(has_failure(&failures, "perf"));
        assert!(has_failure(&failures, "zed-sum-tree"));
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
    fn manifest_scan_allows_current_zed_forks() {
        let failures = manifest_failures(
            r#"
[dependencies]
reqwest = { git = 'https://github.com/zed-industries/reqwest.git', package = 'zed-reqwest', version = '0.12.15-zed' }
wgpu = { git = 'https://github.com/zed-industries/wgpu.git' }
"#,
        );

        assert!(
            failures.is_empty(),
            "expected current Zed-maintained forks to be allowed, got: {failures:?}"
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
name = "allowed"
version = "0.1.0"
source = "git+https://github.com/zed-industries/zed.git?rev=abc#123"
"#,
        );

        assert!(has_failure(&failures, "perf"));
        assert!(has_failure(&failures, "zed-industries/zed.git"));
    }
}
