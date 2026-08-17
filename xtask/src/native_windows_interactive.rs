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

#[derive(Default)]
struct NativeGateReport {
    failed_steps: Vec<String>,
}

impl NativeGateReport {
    fn run(&mut self, label: impl Into<String>, step: impl FnOnce() -> Result<(), ()>) {
        let label = label.into();
        if step().is_err() {
            self.failed_steps.push(label);
        }
    }

    #[cfg(test)]
    fn failed_steps(&self) -> &[String] {
        &self.failed_steps
    }

    fn finish(self) -> Result<(), ()> {
        if self.failed_steps.is_empty() {
            return Ok(());
        }

        eprintln!("native Windows interactive gate failed steps:");
        for step in self.failed_steps {
            eprintln!("- {step}");
        }
        Err(())
    }
}

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
    pub(crate) behavior: NativeScenarioBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NativeScenarioBehavior {
    SourceCapture,
    OpaqueOcclusion,
    SurfaceShutdown,
    ProvisionalSameHwndPromotion,
    LiveRouteAndReleaseLock,
    CommittedLossRecovery,
    ProcessConvergence,
    NoInputPassThrough,
    MixedDpiPlacement,
}

impl NativeScenarioBehavior {
    const ALL: [Self; 9] = [
        Self::SourceCapture,
        Self::OpaqueOcclusion,
        Self::SurfaceShutdown,
        Self::ProvisionalSameHwndPromotion,
        Self::LiveRouteAndReleaseLock,
        Self::CommittedLossRecovery,
        Self::ProcessConvergence,
        Self::NoInputPassThrough,
        Self::MixedDpiPlacement,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SourceCapture => "source-capture",
            Self::OpaqueOcclusion => "opaque-occlusion",
            Self::SurfaceShutdown => "surface-shutdown",
            Self::ProvisionalSameHwndPromotion => "provisional-same-hwnd-promotion",
            Self::LiveRouteAndReleaseLock => "live-route-and-release-lock",
            Self::CommittedLossRecovery => "committed-loss-recovery",
            Self::ProcessConvergence => "process-convergence",
            Self::NoInputPassThrough => "no-input-pass-through",
            Self::MixedDpiPlacement => "mixed-dpi-placement",
        }
    }

    const fn requirement_owner(self) -> &'static str {
        match self {
            Self::SourceCapture | Self::OpaqueOcclusion | Self::SurfaceShutdown => "U27",
            Self::ProcessConvergence | Self::NoInputPassThrough | Self::MixedDpiPlacement => "U28",
            Self::ProvisionalSameHwndPromotion
            | Self::LiveRouteAndReleaseLock
            | Self::CommittedLossRecovery => "U29",
        }
    }

    const fn test(self) -> &'static str {
        match self {
            Self::SourceCapture => {
                "native_interactive_tests::native_interactive_two_hwnd_captured_drag_routes_preview_and_drop"
            }
            Self::OpaqueOcclusion => {
                "native_interactive_tests::native_interactive_opaque_occlusion_blocks_underlay_and_preserves_same_hwnd"
            }
            Self::SurfaceShutdown => {
                "native_interactive_tests::native_interactive_anchor_close_releases_capture_and_retires_dependent_hwnds"
            }
            Self::ProvisionalSameHwndPromotion => {
                "native_interactive_tests::native_interactive_provisional_gate_presents_and_promotes_same_hwnd"
            }
            Self::LiveRouteAndReleaseLock => {
                "native_interactive_tests::native_interactive_live_route_reuses_same_hwnd_and_locks_release"
            }
            Self::CommittedLossRecovery => {
                "native_interactive_tests::native_interactive_committed_destination_loss_retires_runtime_authority"
            }
            Self::ProcessConvergence => {
                "native_interactive_tests::native_interactive_process_converges_after_active_surface_shutdown"
            }
            Self::NoInputPassThrough => {
                "native_interactive_tests::native_interactive_no_input_prefix_passes_through_and_fails_closed_on_generation_drift"
            }
            Self::MixedDpiPlacement => {
                "native_interactive_tests::native_interactive_mixed_dpi_final_client_bounds_are_exact"
            }
        }
    }
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
    let mut behaviors = BTreeMap::<NativeScenarioBehavior, &str>::new();
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
        if scenario.requirement_owner != scenario.behavior.requirement_owner() {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` behavior `{}` belongs to {}, not {}",
                scenario.id,
                scenario.behavior.as_str(),
                scenario.behavior.requirement_owner(),
                scenario.requirement_owner,
            ));
        }
        if scenario.test != scenario.behavior.test() {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenario `{}` behavior `{}` must use exact test coordinate `{}`",
                scenario.id,
                scenario.behavior.as_str(),
                scenario.behavior.test(),
            ));
        }
        if let Some(previous) = behaviors.insert(scenario.behavior, &scenario.id) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenarios `{previous}` and `{}` share behavior `{}`; one behavior cannot dispatch two manifest scenarios",
                scenario.id,
                scenario.behavior.as_str(),
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
    for required in NativeScenarioBehavior::ALL {
        if !behaviors.contains_key(&required) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: missing required native behavior `{}`",
                required.as_str()
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

    let mut report = NativeGateReport::default();
    for scenario in &manifest.scenario {
        println!(
            "==> native scenario {} ({})",
            scenario.id, scenario.requirement_owner
        );
        report.run(format!("native scenario {}", scenario.id), || {
            run_exact_test(
                root,
                NATIVE_DOCK_PACKAGE,
                &scenario.test,
                true,
                Some(&scenario.id),
            )
        });
    }
    report.run("open-gpui-docking-native baseline", || {
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
    });
    report.finish()
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
    use std::cell::RefCell;

    #[test]
    fn native_gate_report_runs_every_step_before_returning_failure() {
        let executed = RefCell::new(Vec::new());
        let mut report = NativeGateReport::default();

        report.run("sentinel", || {
            executed.borrow_mut().push("sentinel");
            Err(())
        });
        report.run("later scenario", || {
            executed.borrow_mut().push("later scenario");
            Ok(())
        });

        assert_eq!(executed.into_inner(), ["sentinel", "later scenario"]);
        assert_eq!(report.failed_steps(), ["sentinel"]);
        assert_eq!(report.finish(), Err(()));
    }

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
    fn manifest_rejects_a_missing_required_behavior() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask must remain a direct workspace child");
        let mut manifest = load_native_scenario_manifest(root).expect("manifest should parse");
        manifest
            .scenario
            .retain(|scenario| scenario.behavior != NativeScenarioBehavior::ProcessConvergence);

        let failures = native_manifest_failures(&manifest).join("\n");
        assert!(
            failures.contains("missing required native behavior `process-convergence`"),
            "missing fail-closed behavior diagnostic: {failures}"
        );
    }
}
