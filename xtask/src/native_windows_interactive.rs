use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
    process::Command,
};

use serde::Deserialize;

use crate::ui_contract::scan_ui_contract;

pub(crate) const NATIVE_SCENARIO_MANIFEST_PATH: &str =
    "examples/docking-native/tests/native_windows_interactive.native-scenarios.toml";
pub(crate) const NATIVE_SCENARIO_SUITE: &str = "docking-native.windows.interactive";
pub(crate) const NATIVE_SCENARIO_RUNNER: &str = "open-gpui-windows-native-interactive-ephemeral";
pub(crate) const NATIVE_SCENARIO_WORKFLOW_ENTRY: &str =
    "cargo run --locked -p xtask -- native-windows-interactive";

const NATIVE_DOCK_PACKAGE: &str = "open-gpui-docking-native";
const WINDOWS_PACKAGE: &str = "open-gpui-windows";
const NATIVE_SCENARIO_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_ID";
const NATIVE_RUNNER_SENTINEL_TEST: &str = "platform::native_test_support::native_interactive_runner_sentinel_proves_system_pointer_delivery_and_capture";
const NATIVE_SCENARIO_REGISTRY_TEST: &str =
    "native_interactive_tests::native_interactive_scenario_registry_matches_cases";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeScenarioManifest {
    pub(crate) schema: u16,
    pub(crate) suite: String,
    pub(crate) runner: String,
    pub(crate) scenario: Vec<NativeScenarioDeclaration>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeScenarioDeclaration {
    pub(crate) id: String,
    pub(crate) requirement_owner: String,
    pub(crate) test: String,
    pub(crate) observation_domains: BTreeSet<NativeObservationDomain>,
    pub(crate) behavior: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NativeObservationDomain {
    VisualTest,
    SystemInput,
    WndProc,
    Capture,
    PointStack,
    Presentation,
    Lifetime,
}

impl NativeObservationDomain {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::VisualTest => "visual-test",
            Self::SystemInput => "system-input",
            Self::WndProc => "wnd-proc",
            Self::Capture => "capture",
            Self::PointStack => "point-stack",
            Self::Presentation => "presentation",
            Self::Lifetime => "lifetime",
        }
    }
}

pub(crate) fn load_native_scenario_manifest(root: &Path) -> Result<NativeScenarioManifest, String> {
    let source = fs::read_to_string(root.join(NATIVE_SCENARIO_MANIFEST_PATH)).map_err(|error| {
        format!("{NATIVE_SCENARIO_MANIFEST_PATH}: failed to read native scenario manifest: {error}")
    })?;
    toml::from_str(&source).map_err(|error| {
        format!("{NATIVE_SCENARIO_MANIFEST_PATH}: invalid native scenario manifest: {error}")
    })
}

pub(crate) fn native_manifest_failures(manifest: &NativeScenarioManifest) -> Vec<String> {
    let mut failures = Vec::new();
    if manifest.schema != 3 {
        failures.push(format!(
            "{NATIVE_SCENARIO_MANIFEST_PATH}: unsupported native scenario schema {}; expected 3",
            manifest.schema
        ));
    }
    if manifest.suite != NATIVE_SCENARIO_SUITE {
        failures.push(format!(
            "{NATIVE_SCENARIO_MANIFEST_PATH}: suite `{}` must be `{NATIVE_SCENARIO_SUITE}`",
            manifest.suite
        ));
    }
    if manifest.runner != NATIVE_SCENARIO_RUNNER {
        failures.push(format!(
            "{NATIVE_SCENARIO_MANIFEST_PATH}: runner `{}` must be `{NATIVE_SCENARIO_RUNNER}`",
            manifest.runner
        ));
    }
    if manifest.scenario.is_empty() {
        failures.push(format!(
            "{NATIVE_SCENARIO_MANIFEST_PATH}: at least one native scenario is required"
        ));
    }

    let mut ids = BTreeSet::new();
    let mut tests = BTreeMap::<&str, &str>::new();
    let mut behaviors = BTreeMap::<&str, &str>::new();
    for scenario in &manifest.scenario {
        if scenario.id.trim().is_empty() {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario id cannot be empty"
            ));
        } else if !ids.insert(scenario.id.as_str()) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` is duplicated",
                scenario.id
            ));
        }
        if !matches!(scenario.requirement_owner.as_str(), "U27" | "U28" | "U29") {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` requirement owner `{}` must be U27, U28, or U29",
                scenario.id, scenario.requirement_owner
            ));
        }
        let expected_prefix = format!(
            "native.{}.",
            scenario.requirement_owner.to_ascii_lowercase()
        );
        if !scenario.id.starts_with(&expected_prefix) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` does not match requirement owner `{}`",
                scenario.id, scenario.requirement_owner
            ));
        }
        if !is_valid_test_coordinate(&scenario.test) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` test coordinate `{}` is not an exact ASCII Rust test path",
                scenario.id, scenario.test
            ));
        }
        if let Some(previous) = tests.insert(&scenario.test, &scenario.id) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenarios `{previous}` and `{}` share test coordinate `{}`; one alias worker cannot satisfy two native scenarios",
                scenario.id, scenario.test
            ));
        }
        if !is_valid_behavior_key(&scenario.behavior) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` behavior `{}` is not an exact ASCII kebab-case behavior key",
                scenario.id, scenario.behavior
            ));
        }
        if let Some(previous) = behaviors.insert(&scenario.behavior, &scenario.id) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenarios `{previous}` and `{}` share behavior `{}`; one behavior cannot dispatch two manifest scenarios",
                scenario.id, scenario.behavior
            ));
        }
        if scenario.observation_domains.is_empty() {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` must declare at least one typed observation domain",
                scenario.id
            ));
        }
        if scenario
            .observation_domains
            .contains(&NativeObservationDomain::VisualTest)
        {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` declares `visual-test`; VisualTest evidence cannot satisfy an owning-platform native gate",
                scenario.id
            ));
        }
    }
    failures
}

pub(crate) fn native_windows_interactive(root: &Path) -> Result<(), ()> {
    if !cfg!(target_os = "windows") {
        eprintln!("native-windows-interactive requires a Windows host");
        return Err(());
    }
    if env::var("OPEN_GPUI_NATIVE_INTERACTIVE").ok().as_deref() != Some("1") {
        eprintln!(
            "native-windows-interactive requires OPEN_GPUI_NATIVE_INTERACTIVE=1 on the dedicated interactive runner"
        );
        return Err(());
    }

    let manifest = load_native_scenario_manifest(root).map_err(|error| eprintln!("{error}"))?;
    let failures = native_manifest_failures(&manifest);
    if !failures.is_empty() {
        for failure in failures {
            eprintln!("{failure}");
        }
        return Err(());
    }
    scan_ui_contract(root)?;

    run_exact_test(
        root,
        WINDOWS_PACKAGE,
        NATIVE_RUNNER_SENTINEL_TEST,
        true,
        None,
    )?;
    run_cargo(
        root,
        [
            "nextest",
            "run",
            "-p",
            WINDOWS_PACKAGE,
            "--locked",
            "--test-threads=1",
            "--no-fail-fast",
        ],
    )?;
    run_exact_test(
        root,
        NATIVE_DOCK_PACKAGE,
        NATIVE_SCENARIO_REGISTRY_TEST,
        false,
        None,
    )?;
    for scenario in &manifest.scenario {
        println!(
            "==> native scenario {} ({})",
            scenario.id, scenario.requirement_owner
        );
        run_exact_test(
            root,
            NATIVE_DOCK_PACKAGE,
            &scenario.test,
            true,
            Some(&scenario.id),
        )?;
    }
    run_cargo(
        root,
        [
            "nextest",
            "run",
            "-p",
            NATIVE_DOCK_PACKAGE,
            "--locked",
            "--test-threads=1",
            "--no-fail-fast",
        ],
    )
}

fn run_exact_test(
    root: &Path,
    package: &str,
    test: &str,
    ignored: bool,
    scenario_id: Option<&str>,
) -> Result<(), ()> {
    let selector = format!("test(={test})");
    let mut args = vec![
        "nextest".to_owned(),
        "run".to_owned(),
        "-p".to_owned(),
        package.to_owned(),
        "--locked".to_owned(),
    ];
    if ignored {
        args.extend(["--run-ignored".to_owned(), "only".to_owned()]);
    }
    args.extend([
        "-E".to_owned(),
        selector,
        "--test-threads=1".to_owned(),
        "--no-fail-fast".to_owned(),
        "--no-tests".to_owned(),
        "fail".to_owned(),
    ]);
    run_cargo_with_scenario(root, args, scenario_id)
}

fn run_cargo(root: &Path, args: impl IntoIterator<Item = impl Into<String>>) -> Result<(), ()> {
    run_cargo_with_scenario(root, args, None)
}

fn run_cargo_with_scenario(
    root: &Path,
    args: impl IntoIterator<Item = impl Into<String>>,
    scenario_id: Option<&str>,
) -> Result<(), ()> {
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    println!("==> cargo {}", args.join(" "));
    let mut command = Command::new("cargo");
    command.args(&args).current_dir(root);
    if let Some(scenario_id) = scenario_id {
        command.env(NATIVE_SCENARIO_ENV, scenario_id);
    }
    let status = command.status().map_err(|error| {
        eprintln!("failed to run `cargo {}`: {error}", args.join(" "));
    })?;
    if status.success() {
        Ok(())
    } else {
        eprintln!("command failed: cargo {}", args.join(" "));
        Err(())
    }
}

fn is_valid_behavior_key(behavior: &str) -> bool {
    !behavior.is_empty()
        && !behavior.starts_with('-')
        && !behavior.ends_with('-')
        && behavior.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn is_valid_test_coordinate(test: &str) -> bool {
    !test.is_empty()
        && !test.starts_with("::")
        && !test.ends_with("::")
        && test.split("::").all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_test_coordinates_reject_filters_and_shell_syntax() {
        for invalid in [
            "",
            "::test",
            "test::",
            "test(/worker/)",
            "test::*",
            "test name",
            "test;exit 0",
        ] {
            assert!(!is_valid_test_coordinate(invalid), "accepted `{invalid}`");
        }
        assert!(is_valid_test_coordinate(
            "native_interactive_tests::native_interactive_worker"
        ));
    }

    #[test]
    fn behavior_keys_are_exact_kebab_case() {
        for invalid in ["", "-source", "source-", "source--capture", "SourceCapture"] {
            assert!(!is_valid_behavior_key(invalid), "accepted `{invalid}`");
        }
        assert!(is_valid_behavior_key("no-input-pass-through"));
    }
}
