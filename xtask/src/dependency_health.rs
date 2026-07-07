use std::{
    collections::{BTreeMap, BTreeSet},
    process::{Command, Stdio},
};

use serde_json::Value;

use std::path::Path;

const DECLARED_MSRV: &str = "1.92";

const ALLOWED_DUPLICATE_CRATES: &[&str] = &[
    "accesskit_consumer",
    "bindgen",
    "bit-set",
    "bit-vec",
    "bitflags",
    "block2",
    "convert_case",
    "core-foundation",
    "core-graphics",
    "core-graphics-types",
    "cpufeatures",
    "getrandom",
    "hashbrown",
    "heapless",
    "itertools",
    "jni",
    "jni-sys",
    "linux-raw-sys",
    "nix",
    "nom",
    "objc2",
    "objc2-app-kit",
    "objc2-foundation",
    "objc2-metal",
    "objc2-quartz-core",
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
    "rustls-platform-verifier",
    "serde_spanned",
    "shlex",
    "skrifa",
    "spin",
    "thiserror",
    "thiserror-impl",
    "toml",
    "toml_datetime",
    "toml_edit",
    "unicode-width",
    "webpki-root-certs",
    "windows",
    "windows-collections",
    "windows-core",
    "windows-future",
    "windows-implement",
    "windows-interface",
    "windows-link",
    "windows-numerics",
    "windows-result",
    "windows-strings",
    "windows-sys",
    "windows-targets",
    "windows-threading",
    "windows_aarch64_gnullvm",
    "windows_aarch64_msvc",
    "windows_i686_gnu",
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
    let declared = RustVersion::parse(DECLARED_MSRV).expect("declared MSRV must parse");
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();

    let mut max_dependency_version = RustVersion::default();
    let mut max_dependency_packages = Vec::new();

    for package in packages(metadata) {
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let rust_version = package.get("rust_version").and_then(Value::as_str);

        if workspace_members.contains(id) {
            if rust_version != Some(DECLARED_MSRV) {
                failures.push(format!(
                    "workspace package `{name}` must inherit rust-version `{DECLARED_MSRV}`, found `{}`",
                    rust_version.unwrap_or("missing")
                ));
            }
        }

        if let Some(rust_version) = rust_version.and_then(RustVersion::parse) {
            match rust_version.cmp(&max_dependency_version) {
                std::cmp::Ordering::Greater => {
                    max_dependency_version = rust_version;
                    max_dependency_packages.clear();
                    max_dependency_packages.push(name.to_string());
                }
                std::cmp::Ordering::Equal => max_dependency_packages.push(name.to_string()),
                std::cmp::Ordering::Less => {}
            }
        }
    }

    if max_dependency_version > declared {
        failures.push(format!(
            "declared MSRV `{DECLARED_MSRV}` is below dependency floor `{}` from {}",
            max_dependency_version,
            max_dependency_packages.join(", ")
        ));
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
