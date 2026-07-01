use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

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

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::FAILURE;
    };

    let root = workspace_root();
    let result = match command.as_str() {
        "verify" => verify(&root),
        "renderer-smoke" => renderer_smoke(&root),
        "scan-theme-drift" => scan_theme_drift(&root),
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
    eprintln!("  renderer-smoke        run the native wgpu renderer smoke test");
    eprintln!("  scan-theme-drift      scan theme token and recipe drift");
    eprintln!("  scan-import-boundary  scan for disallowed import residue");
}

fn verify(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["fmt", "--all", "--check"])?;
    run(root, "cargo", &["check", "--workspace"])?;
    run(root, "cargo", &["check", "-p", "open-gpui-smoke-native"])?;
    run_ui_component_tests(root)?;
    scan_theme_drift(root)?;
    scan_import_boundary(root)?;
    Ok(())
}

fn run_ui_component_tests(root: &Path) -> Result<(), ()> {
    for package in [
        "open-gpui-ui-core",
        "open-gpui-ui-components",
        "open-gpui-ui-foundation-gallery",
    ] {
        run(root, "cargo", &["nextest", "run", "-p", package])?;
    }

    Ok(())
}

fn renderer_smoke(root: &Path) -> Result<(), ()> {
    run(
        root,
        "cargo",
        &[
            "nextest",
            "run",
            "-p",
            "open-gpui-wgpu",
            "--features",
            "font-kit",
            "renderer_smoke_creates_core_pipelines",
        ],
    )
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

fn scan_theme_drift(root: &Path) -> Result<(), ()> {
    println!("==> scan theme drift");

    let mut failures = Vec::new();
    scan_theme_recipe_coverage(root, &mut failures);
    scan_theme_palette_coverage(root, &mut failures);

    if failures.is_empty() {
        println!("theme drift scan passed");
        Ok(())
    } else {
        eprintln!("theme drift scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn scan_theme_recipe_coverage(root: &Path, failures: &mut Vec<String>) {
    let theme_dir = root.join("crates/ui_components/src/theme");
    let recipes_path = theme_dir.join("recipes.rs");
    let Ok(recipes) = fs::read_to_string(&recipes_path) else {
        failures.push("crates/ui_components/src/theme/recipes.rs: failed to read file".to_string());
        return;
    };

    let recipe_definitions = recipe_definitions(&recipes);
    let recipe_calls = theme_recipe_calls(root);
    let recipe_catalog = theme_recipe_catalog(&recipes);

    for recipe in &recipe_calls {
        if !recipe_definitions.contains(recipe) {
            failures.push(format!(
                "theme recipe `{recipe}` is called by a component but has no `ThemeResolver::{recipe}` definition in theme/recipes.rs"
            ));
        }
        if !recipe_catalog.contains(recipe) {
            failures.push(format!(
                "theme recipe `{recipe}` is missing from `THEME_RECIPE_CATALOG` in theme/recipes.rs"
            ));
        }
    }

    for recipe in recipe_definitions {
        if !recipe_catalog.contains(&recipe) {
            failures.push(format!(
                "theme recipe `{recipe}` is defined but missing from `THEME_RECIPE_CATALOG`"
            ));
        }
    }

    for path in tracked_text_files(root, is_ui_component_rust_file) {
        if path.starts_with(&theme_dir) {
            continue;
        }

        let relative = display_path(root, &path);
        let Ok(contents) = fs::read_to_string(&path) else {
            failures.push(format!("{relative}: failed to read file"));
            continue;
        };

        if contents.contains("impl ThemeResolver") {
            failures.push(format!(
                "{relative}: component files must not extend ThemeResolver; move color recipes to crates/ui_components/src/theme/recipes.rs"
            ));
        }
    }
}

fn scan_theme_palette_coverage(root: &Path, failures: &mut Vec<String>) {
    let path = root.join("crates/ui_components/src/theme/palette.rs");
    let relative = "crates/ui_components/src/theme/palette.rs";
    let Ok(contents) = fs::read_to_string(&path) else {
        failures.push(format!("{relative}: failed to read file"));
        return;
    };

    let themes = theme_palette_entries(&contents);
    for required in [
        "LIGHT_THEME_COLORS",
        "DARK_THEME_COLORS",
        "HIGH_CONTRAST_THEME_COLORS",
    ] {
        if !themes.contains_key(required) {
            failures.push(format!("{relative}: missing `{required}` color table"));
        }
    }

    let Some(light) = themes.get("LIGHT_THEME_COLORS") else {
        return;
    };

    for (name, entries) in &themes {
        if entries != light {
            let missing = light.difference(entries).cloned().collect::<Vec<_>>();
            let extra = entries.difference(light).cloned().collect::<Vec<_>>();
            if !missing.is_empty() {
                failures.push(format!(
                    "{relative}: `{name}` is missing token/state entries: {}",
                    missing.join(", ")
                ));
            }
            if !extra.is_empty() {
                failures.push(format!(
                    "{relative}: `{name}` has extra token/state entries: {}",
                    extra.join(", ")
                ));
            }
        }
    }
}

fn recipe_definitions(contents: &str) -> BTreeSet<String> {
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with("pub(crate)") {
                return None;
            }
            let start = line.find("fn ")? + 3;
            let name = line[start..].split_once('(')?.0.trim();
            name.ends_with("_colors").then(|| name.to_string())
        })
        .collect()
}

fn theme_recipe_calls(root: &Path) -> BTreeSet<String> {
    let mut calls = BTreeSet::new();
    for path in tracked_text_files(root, is_ui_component_rust_file) {
        if path
            .strip_prefix(root)
            .ok()
            .is_some_and(|relative| relative.starts_with("crates/ui_components/src/theme"))
        {
            continue;
        }

        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        for name in theme_resolver_method_names(&contents) {
            if name.ends_with("_colors") {
                calls.insert(name);
            }
        }
    }
    calls
}

fn theme_recipe_catalog(contents: &str) -> BTreeSet<String> {
    let Some(start) = contents.find("THEME_RECIPE_CATALOG") else {
        return BTreeSet::new();
    };
    let Some(open) = contents[start..].find('[').map(|index| start + index) else {
        return BTreeSet::new();
    };
    let Some(close) = contents[open..].find("];").map(|index| open + index) else {
        return BTreeSet::new();
    };

    quoted_strings(&contents[open..close]).into_iter().collect()
}

fn theme_resolver_method_names(contents: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = contents;
    while let Some(index) = rest.find("ThemeResolver::") {
        rest = &rest[index + "ThemeResolver::".len()..];
        let name = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}

fn theme_palette_entries(contents: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut themes = BTreeMap::new();
    for name in [
        "LIGHT_THEME_COLORS",
        "DARK_THEME_COLORS",
        "HIGH_CONTRAST_THEME_COLORS",
    ] {
        if let Some(body) = const_slice_body(contents, name) {
            themes.insert(name.to_string(), theme_color_entries(body));
        }
    }
    themes
}

fn const_slice_body<'a>(contents: &'a str, name: &str) -> Option<&'a str> {
    let start = contents.find(name)?;
    let open = contents[start..]
        .find("&[")
        .map(|index| start + index + 2)?;
    let close = contents[open..].find("];").map(|index| open + index)?;
    Some(&contents[open..close])
}

fn theme_color_entries(body: &str) -> BTreeSet<String> {
    let mut entries = BTreeSet::new();
    let mut rest = body;
    while let Some(index) = rest.find("ThemeColor::new(") {
        rest = &rest[index + "ThemeColor::new(".len()..];
        let Some(close) = rest.find(')') else {
            break;
        };
        let args = &rest[..close];
        let mut parts = args.split(',').map(str::trim);
        let token = parts.next().unwrap_or_default();
        let state = parts.next().unwrap_or_default();
        if !token.is_empty() && !state.is_empty() {
            entries.insert(format!("{token}/{state}"));
        }
        rest = &rest[close + 1..];
    }
    entries
}

fn quoted_strings(contents: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = contents;
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
            } else if is_disallowed_zed_git_source(git_source) {
                failures.push(format!(
                    "{relative}:{section}.{alias}: disallowed retired Zed fork git source `{git_source}`"
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

fn is_ui_component_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "rs")
        && path.components().any(|component| {
            component.as_os_str() == "ui_components"
                || component.as_os_str() == "crates\\ui_components"
        })
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
    fn theme_recipe_scan_tracks_public_component_recipes_only() {
        let contents = r#"
const THEME_RECIPE_CATALOG: &[&str] = &[
    "button_colors",
];

impl ThemeResolver {
    pub(crate) const fn button_colors(tokens: ThemeTokens) -> ButtonColors {
        Self::accent_button_colors(tokens)
    }

    const fn accent_button_colors(tokens: ThemeTokens) -> ButtonColors {
        todo!()
    }
}
"#;

        assert_eq!(
            recipe_definitions(contents),
            BTreeSet::from(["button_colors".to_string()])
        );
        assert_eq!(
            theme_recipe_catalog(contents),
            BTreeSet::from(["button_colors".to_string()])
        );
    }

    #[test]
    fn theme_palette_scan_compares_token_state_shape() {
        let contents = r#"
const LIGHT_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xf1f5ee),
];

const DARK_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0x000000),
];

const HIGH_CONTRAST_THEME_COLORS: &[ThemeColor] = &[
    ThemeColor::new(semantic::SURFACE, ColorState::Default, 0xffffff),
    ThemeColor::new(semantic::SURFACE_MUTED, ColorState::Hover, 0xffffff),
];
"#;
        let themes = theme_palette_entries(contents);

        assert!(themes["LIGHT_THEME_COLORS"].contains("semantic::SURFACE/ColorState::Default"));
        assert!(
            themes["LIGHT_THEME_COLORS"]
                .difference(&themes["DARK_THEME_COLORS"])
                .any(|entry| entry == "semantic::SURFACE_MUTED/ColorState::Hover")
        );
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
