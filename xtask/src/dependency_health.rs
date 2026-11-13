use std::{
    collections::{BTreeMap, BTreeSet},
    process::{Command, Stdio},
};

use serde_json::Value;

use std::path::Path;

const DECLARED_MSRV: &str = "1.93";

const MSRV_EXCEPTIONS: &[(&str, &str)] = &[
    ("open-gpui-devtools", "1.95"),
    ("open-gpui-docking", "1.95"),
    ("open-gpui-docking-minimal", "1.95"),
    ("open-gpui-docking-multiviewport", "1.95"),
    ("open-gpui-docking-native", "1.95"),
    ("open-gpui-dockspace-editor", "1.95"),
    ("open-gpui-ui-foundation-gallery", "1.95"),
    ("xtask", "1.95"),
];

const ALLOWED_DUPLICATE_CRATES: &[&str] = &[
    "accesskit_consumer",
    "async-channel",
    "base64",
    "bitflags",
    "block-buffer",
    "block2",
    "convert_case",
    "core-foundation",
    "core-graphics",
    "core-graphics-types",
    "cpufeatures",
    "crypto-common",
    "digest",
    "event-listener",
    "getrandom",
    "hashbrown",
    "heapless",
    "itertools",
    "jni-sys",
    "linux-raw-sys",
    "nix",
    "nom",
    "objc2",
    "objc2-app-kit",
    "objc2-foundation",
    "objc2-metal",
    "objc2-quartz-core",
    "pastey",
    "png",
    "pollster",
    "quick-error",
    "r-efi",
    "rand",
    "rand_chacha",
    "rand_core",
    "read-fonts",
    "redox_syscall",
    "roxmltree",
    "rustc-hash",
    "rustix",
    "serde_spanned",
    "sha2",
    "shlex",
    "skrifa",
    "spin",
    "syn",
    "thiserror",
    "thiserror-impl",
    "toml",
    "toml_datetime",
    "toml_edit",
    "unicode-width",
    "windows",
    "windows-collections",
    "windows-core",
    "windows-future",
    "windows-implement",
    "windows-interface",
    "windows-link",
    "windows-numerics",
    "windows-registry",
    "windows-result",
    "windows-strings",
    "windows-sys",
    "windows-targets",
    "windows-threading",
    "windows_aarch64_gnullvm",
    "windows_aarch64_msvc",
    "windows_i686_gnu",
    "windows_i686_gnullvm",
    "windows_i686_msvc",
    "windows_x86_64_gnu",
    "windows_x86_64_gnullvm",
    "windows_x86_64_msvc",
    "winnow",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RustVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

pub(crate) fn dependency_health(root: &Path) -> Result<(), ()> {
    println!("==> dependency health");

    let metadata = cargo_metadata(root)?;
    let mut failures = Vec::new();
    failures.extend(msrv_failures(&metadata));
    failures.extend(duplicate_dependency_failures(&metadata));
    failures.extend(cargo_audit_failures(root));

    if failures.is_empty() {
        println!("dependency health passed");
        Ok(())
    } else {
        eprintln!("dependency health failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn cargo_metadata(root: &Path) -> Result<Value, ()> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|error| {
            eprintln!("failed to run cargo metadata: {error}");
        })?;

    if !output.status.success() {
        eprintln!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(());
    }

    serde_json::from_slice(&output.stdout).map_err(|error| {
        eprintln!("cargo metadata returned invalid JSON: {error}");
    })
}

fn msrv_failures(metadata: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let packages_by_id = packages(metadata)
        .filter_map(|package| Some((package.get("id")?.as_str()?, package)))
        .collect::<BTreeMap<_, _>>();
    let Some(nodes) = metadata.pointer("/resolve/nodes").and_then(Value::as_array) else {
        return vec![
            "cargo metadata must include resolved dependency nodes for MSRV checks".into(),
        ];
    };
    let dependencies_by_id = nodes
        .iter()
        .filter_map(|node| {
            Some((
                node.get("id")?.as_str()?,
                node.get("dependencies")?.as_array()?,
            ))
        })
        .collect::<BTreeMap<_, _>>();

    for package in packages(metadata) {
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !workspace_members.contains(id) {
            continue;
        }

        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let expected = MSRV_EXCEPTIONS
            .iter()
            .find_map(|&(exception, version)| (exception == name).then_some(version))
            .unwrap_or(DECLARED_MSRV);
        let rust_version = package.get("rust_version").and_then(Value::as_str);
        if rust_version != Some(expected) {
            failures.push(format!(
                "workspace package `{name}` must declare rust-version `{expected}`, found `{}`",
                rust_version.unwrap_or("missing")
            ));
        }
        let Some(declared) = rust_version.and_then(RustVersion::parse) else {
            continue;
        };
        if !dependencies_by_id.contains_key(id) {
            failures.push(format!(
                "workspace package `{name}` is missing from resolved dependency nodes"
            ));
            continue;
        }

        let mut pending = vec![id];
        let mut visited = BTreeSet::new();
        let mut max_dependency_version = RustVersion::default();
        let mut max_dependency_packages = BTreeSet::new();
        while let Some(dependency_id) = pending.pop() {
            if !visited.insert(dependency_id) {
                continue;
            }
            if let Some(dependencies) = dependencies_by_id.get(dependency_id) {
                pending.extend(dependencies.iter().filter_map(Value::as_str));
            }
            let Some(dependency) = packages_by_id.get(dependency_id) else {
                continue;
            };
            let Some(version) = dependency
                .get("rust_version")
                .and_then(Value::as_str)
                .and_then(RustVersion::parse)
            else {
                continue;
            };
            let dependency_name = dependency
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            match version.cmp(&max_dependency_version) {
                std::cmp::Ordering::Greater => {
                    max_dependency_version = version;
                    max_dependency_packages.clear();
                    max_dependency_packages.insert(dependency_name);
                }
                std::cmp::Ordering::Equal => {
                    max_dependency_packages.insert(dependency_name);
                }
                std::cmp::Ordering::Less => {}
            }
        }

        if max_dependency_version > declared {
            failures.push(format!(
                "workspace package `{name}` declares MSRV `{declared}` below dependency floor `{max_dependency_version}` from {}",
                max_dependency_packages.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    failures
}

fn duplicate_dependency_failures(metadata: &Value) -> Vec<String> {
    let mut by_name = BTreeMap::<String, BTreeSet<String>>::new();
    for package in packages(metadata) {
        let Some(source) = package.get("source").and_then(Value::as_str) else {
            continue;
        };
        if !source.starts_with("registry+") {
            continue;
        }
        let Some(name) = package.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = package.get("version").and_then(Value::as_str) else {
            continue;
        };
        by_name
            .entry(name.to_string())
            .or_default()
            .insert(version.to_string());
    }

    let allowed = ALLOWED_DUPLICATE_CRATES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual_duplicates = by_name
        .iter()
        .filter(|(_, versions)| versions.len() > 1)
        .map(|(name, versions)| (name.as_str(), versions))
        .collect::<Vec<_>>();

    let mut failures = Vec::new();
    for (name, versions) in &actual_duplicates {
        if !allowed.contains(name) {
            failures.push(format!(
                "unexpected duplicate dependency `{name}` has versions {}",
                versions.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    let actual_names = actual_duplicates
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    for stale in allowed.difference(&actual_names) {
        failures.push(format!(
            "duplicate dependency allowlist entry `{stale}` is stale; remove it"
        ));
    }

    failures
}

fn cargo_audit_failures(root: &Path) -> Vec<String> {
    let output = Command::new("cargo")
        .args(["audit", "--json"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return vec![format!(
                "`cargo audit` is required for dependency health but could not be started: {error}"
            )];
        }
    };

    if output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Ok(report) = serde_json::from_str::<Value>(&stdout) {
        let advisories = report
            .pointer("/vulnerabilities/list")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let id = entry.pointer("/advisory/id").and_then(Value::as_str)?;
                let package = entry.pointer("/package/name").and_then(Value::as_str)?;
                let version = entry.pointer("/package/version").and_then(Value::as_str)?;
                Some(format!("{id} affects {package} {version}"))
            })
            .collect::<Vec<_>>();
        if !advisories.is_empty() {
            return advisories
                .into_iter()
                .map(|advisory| format!("cargo audit vulnerability: {advisory}"))
                .collect();
        }
    }

    vec![format!(
        "cargo audit failed without a parseable vulnerability list: {}{}",
        stdout, stderr
    )]
}

fn packages(metadata: &Value) -> impl Iterator<Item = &Value> {
    metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

impl RustVersion {
    fn parse(version: &str) -> Option<Self> {
        let core = version.split_once('-').map_or(version, |(core, _)| core);
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl Default for RustVersion {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 0,
            patch: 0,
        }
    }
}

impl std::fmt::Display for RustVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.patch == 0 {
            write!(f, "{}.{}", self.major, self.minor)
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_version_parser_accepts_two_or_three_segments() {
        assert_eq!(
            RustVersion::parse("1.92"),
            Some(RustVersion {
                major: 1,
                minor: 92,
                patch: 0
            })
        );
        assert_eq!(
            RustVersion::parse("1.87.0"),
            Some(RustVersion {
                major: 1,
                minor: 87,
                patch: 0
            })
        );
    }

    #[test]
    fn msrv_scan_rejects_transitive_dependency_above_package_floor() {
        let metadata = serde_json::json!({
            "workspace_members": ["core"],
            "packages": [
                {"id": "core", "name": "open-gpui", "rust_version": "1.93"},
                {"id": "helper", "name": "helper", "rust_version": "1.93"},
                {"id": "dependency", "name": "requires-new-rust", "rust_version": "1.95"}
            ],
            "resolve": {"nodes": [
                {"id": "core", "dependencies": ["helper"]},
                {"id": "helper", "dependencies": ["dependency"]},
                {"id": "dependency", "dependencies": []}
            ]}
        });

        let failures = msrv_failures(&metadata);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("`open-gpui` declares MSRV `1.93`"));
        assert!(failures[0].contains("dependency floor `1.95` from requires-new-rust"));
    }

    #[test]
    fn msrv_scan_allows_docking_floor_without_raising_core_floor() {
        let metadata = serde_json::json!({
            "workspace_members": ["core", "docking"],
            "packages": [
                {"id": "core", "name": "open-gpui", "rust_version": "1.93"},
                {"id": "docking", "name": "open-gpui-docking", "rust_version": "1.95"},
                {"id": "dockspace", "name": "dockspace", "rust_version": "1.95"}
            ],
            "resolve": {"nodes": [
                {"id": "core", "dependencies": []},
                {"id": "docking", "dependencies": ["core", "dockspace"]},
                {"id": "dockspace", "dependencies": []}
            ]}
        });

        assert!(msrv_failures(&metadata).is_empty());
    }

    #[test]
    fn msrv_scan_rejects_unlisted_package_floor() {
        let metadata = serde_json::json!({
            "workspace_members": ["unlisted"],
            "packages": [
                {"id": "unlisted", "name": "unlisted-package", "rust_version": "1.95"}
            ],
            "resolve": {"nodes": [
                {"id": "unlisted", "dependencies": []}
            ]}
        });

        let failures = msrv_failures(&metadata);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("`unlisted-package` must declare rust-version `1.93`"));
    }

    #[test]
    fn msrv_scan_requires_exact_exception_floor() {
        let mut metadata = serde_json::json!({
            "workspace_members": ["docking"],
            "packages": [
                {"id": "docking", "name": "open-gpui-docking", "rust_version": "1.93"}
            ],
            "resolve": {"nodes": [
                {"id": "docking", "dependencies": []}
            ]}
        });

        for version in ["1.93", "1.100"] {
            metadata["packages"][0]["rust_version"] = version.into();
            let failures = msrv_failures(&metadata);
            assert_eq!(failures.len(), 1);
            assert!(failures[0].contains("`open-gpui-docking` must declare rust-version `1.95`"));
        }
    }

    #[test]
    fn duplicate_dependency_scan_rejects_unlisted_duplicates() {
        let metadata = serde_json::json!({
            "packages": [
                {"name": "fresh-dup", "version": "1.0.0", "source": "registry+test"},
                {"name": "fresh-dup", "version": "2.0.0", "source": "registry+test"}
            ]
        });
        assert!(
            duplicate_dependency_failures(&metadata)
                .iter()
                .any(|failure| failure.contains("fresh-dup"))
        );
    }
}
