use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
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
const NATIVE_SCENARIO_BEHAVIOR_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_BEHAVIOR";
const NATIVE_SCENARIO_RECEIPT_PATH_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_RECEIPT_PATH";
const NATIVE_SCENARIO_RECEIPT_TOKEN_ENV: &str = "OPEN_GPUI_NATIVE_SCENARIO_RECEIPT_TOKEN";
const NATIVE_RUNNER_SENTINEL_TEST: &str = "platform::native_test_support::native_interactive_runner_sentinel_proves_system_pointer_delivery_and_capture";
const NATIVE_SCENARIO_REGISTRY_TEST: &str =
    "native_interactive_tests::native_interactive_scenario_registry_matches_cases";
static NATIVE_SCENARIO_RECEIPT_NONCE: AtomicU64 = AtomicU64::new(1);

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

struct NativeScenarioRunReceipt {
    path: PathBuf,
    token: String,
    scenario_id: String,
    behavior: String,
}

impl NativeScenarioRunReceipt {
    fn prepare(scenario: &NativeScenarioDeclaration) -> Result<Self, ()> {
        let counter = NATIVE_SCENARIO_RECEIPT_NONCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                eprintln!("native scenario receipt clock is unavailable: {error}");
            })?
            .as_nanos();
        let token = format!("{}-{timestamp}-{counter}", process::id());
        let path = env::temp_dir().join(format!("open-gpui-native-scenario-{token}.receipt"));
        if let Err(error) = fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "failed to clear stale native scenario receipt `{}`: {error}",
                path.display()
            );
            return Err(());
        }
        Ok(Self {
            path,
            token,
            scenario_id: scenario.id.clone(),
            behavior: scenario.behavior.as_str().to_owned(),
        })
    }

    fn apply_environment(&self, command: &mut Command) {
        command
            .env(NATIVE_SCENARIO_ENV, &self.scenario_id)
            .env(NATIVE_SCENARIO_BEHAVIOR_ENV, &self.behavior)
            .env(NATIVE_SCENARIO_RECEIPT_PATH_ENV, &self.path)
            .env(NATIVE_SCENARIO_RECEIPT_TOKEN_ENV, &self.token);
    }

    fn expected_contents(&self) -> String {
        format!("{}\n{}\n{}\n", self.token, self.scenario_id, self.behavior)
    }

    fn verify(&self) -> Result<(), ()> {
        let contents = fs::read_to_string(&self.path).map_err(|error| {
            eprintln!(
                "native scenario `{}` did not publish its behavior receipt at `{}`: {error}",
                self.scenario_id,
                self.path.display()
            );
        })?;
        if contents == self.expected_contents() {
            return Ok(());
        }
        eprintln!(
            "native scenario `{}` published a mismatched behavior receipt: expected behavior `{}`, receipt={contents:?}",
            self.scenario_id, self.behavior
        );
        Err(())
    }
}

impl Drop for NativeScenarioRunReceipt {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "failed to remove native scenario receipt `{}`: {error}",
                self.path.display()
            );
        }
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
    pub(crate) package: NativeScenarioPackage,
    pub(crate) ignored: bool,
    pub(crate) test: String,
    pub(crate) observation_domains: BTreeSet<NativeObservationDomain>,
    pub(crate) behavior: NativeScenarioBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub(crate) enum NativeScenarioPackage {
    #[serde(rename = "open-gpui-docking-native")]
    DockingNative,
    #[serde(rename = "open-gpui-windows")]
    Windows,
}

impl NativeScenarioPackage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DockingNative => NATIVE_DOCK_PACKAGE,
            Self::Windows => WINDOWS_PACKAGE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NativeScenarioBehavior {
    SourceCapture,
    OpaqueOcclusion,
    OpaqueHitTransparentPrefix,
    SurfaceShutdown,
    ProvisionalSameHwndPromotion,
    LiveRouteAndReleaseLock,
    CommittedLossRecovery,
    ActivationTerminal,
    ProvisionalActivationZOrder,
    ClientGeometryReconciliation,
    EventDrivenGeometryWake,
    ProcessConvergence,
    NoInputPassThrough,
    MixedDpiPlacement,
}

impl NativeScenarioBehavior {
    const ALL: [Self; 14] = [
        Self::SourceCapture,
        Self::OpaqueOcclusion,
        Self::OpaqueHitTransparentPrefix,
        Self::SurfaceShutdown,
        Self::ProvisionalSameHwndPromotion,
        Self::LiveRouteAndReleaseLock,
        Self::CommittedLossRecovery,
        Self::ActivationTerminal,
        Self::ProvisionalActivationZOrder,
        Self::ClientGeometryReconciliation,
        Self::EventDrivenGeometryWake,
        Self::ProcessConvergence,
        Self::NoInputPassThrough,
        Self::MixedDpiPlacement,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SourceCapture => "source-capture",
            Self::OpaqueOcclusion => "opaque-occlusion",
            Self::OpaqueHitTransparentPrefix => "opaque-hit-transparent-prefix",
            Self::SurfaceShutdown => "surface-shutdown",
            Self::ProvisionalSameHwndPromotion => "provisional-same-hwnd-promotion",
            Self::LiveRouteAndReleaseLock => "live-route-and-release-lock",
            Self::CommittedLossRecovery => "committed-loss-recovery",
            Self::ActivationTerminal => "activation-terminal",
            Self::ProvisionalActivationZOrder => "provisional-activation-z-order",
            Self::ClientGeometryReconciliation => "client-geometry-reconciliation",
            Self::EventDrivenGeometryWake => "event-driven-geometry-wake",
            Self::ProcessConvergence => "process-convergence",
            Self::NoInputPassThrough => "no-input-pass-through",
            Self::MixedDpiPlacement => "mixed-dpi-placement",
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
    if manifest.schema != 4 {
        failures.push(format!(
            "{NATIVE_SCENARIO_MANIFEST_PATH}: unsupported native scenario schema {}; expected 4",
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
    let mut tests = BTreeMap::<(NativeScenarioPackage, &str), &str>::new();
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
        if let Some(previous) = tests.insert((scenario.package, &scenario.test), &scenario.id) {
            failures.push(format!(
                "{NATIVE_SCENARIO_MANIFEST_PATH}: native scenarios `{previous}` and `{}` share package/test coordinate `{}/{}`; one alias worker cannot satisfy two native scenarios",
                scenario.id,
                scenario.package.as_str(),
                scenario.test
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
            run_manifest_scenario(root, scenario)
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

fn run_manifest_scenario(root: &Path, scenario: &NativeScenarioDeclaration) -> Result<(), ()> {
    run_exact_test(
        root,
        scenario.package.as_str(),
        &scenario.test,
        scenario.ignored,
        Some(scenario),
    )
}

fn run_exact_test(
    root: &Path,
    package: &str,
    test: &str,
    ignored: bool,
    scenario: Option<&NativeScenarioDeclaration>,
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
    run_cargo_with_scenario(root, args, scenario)
}

fn run_cargo(root: &Path, args: impl IntoIterator<Item = impl Into<String>>) -> Result<(), ()> {
    run_cargo_with_scenario(root, args, None)
}

fn run_cargo_with_scenario(
    root: &Path,
    args: impl IntoIterator<Item = impl Into<String>>,
    scenario: Option<&NativeScenarioDeclaration>,
) -> Result<(), ()> {
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    println!("==> cargo {}", args.join(" "));
    let mut command = Command::new("cargo");
    command.args(&args).current_dir(root);
    let receipt = if let Some(scenario) = scenario {
        let receipt = NativeScenarioRunReceipt::prepare(scenario)?;
        receipt.apply_environment(&mut command);
        Some(receipt)
    } else {
        command
            .env_remove(NATIVE_SCENARIO_ENV)
            .env_remove(NATIVE_SCENARIO_BEHAVIOR_ENV)
            .env_remove(NATIVE_SCENARIO_RECEIPT_PATH_ENV)
            .env_remove(NATIVE_SCENARIO_RECEIPT_TOKEN_ENV);
        None
    };
    let status = command.status().map_err(|error| {
        eprintln!("failed to run `cargo {}`: {error}", args.join(" "));
    })?;
    if !status.success() {
        eprintln!("command failed: cargo {}", args.join(" "));
        return Err(());
    }
    receipt
        .as_ref()
        .map_or(Ok(()), NativeScenarioRunReceipt::verify)
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

    #[test]
    fn manifest_remains_the_only_scenario_metadata_authority() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask must remain a direct workspace child");
        let mut manifest = load_native_scenario_manifest(root).expect("manifest should parse");
        let scenario = manifest
            .scenario
            .iter_mut()
            .find(|scenario| {
                scenario.behavior == NativeScenarioBehavior::ClientGeometryReconciliation
            })
            .expect("the repository manifest should own the client-geometry behavior");
        scenario.package = NativeScenarioPackage::DockingNative;
        scenario.ignored = true;

        let failures = native_manifest_failures(&manifest);
        assert!(
            failures.is_empty(),
            "xtask must not duplicate package or ignored metadata outside the manifest: {failures:?}"
        );
    }

    #[test]
    fn behavior_receipt_rejects_redirect_to_another_existing_test() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask must remain a direct workspace child");
        let manifest = load_native_scenario_manifest(root).expect("manifest should parse");
        let scenario = manifest
            .scenario
            .iter()
            .find(|scenario| {
                scenario.behavior == NativeScenarioBehavior::ProvisionalActivationZOrder
            })
            .expect("the repository manifest should own the provisional activation behavior");
        let receipt = NativeScenarioRunReceipt::prepare(scenario)
            .expect("the behavior receipt should reserve one temporary path");

        assert_eq!(
            receipt.verify(),
            Err(()),
            "an unrelated test that publishes no receipt must not satisfy the scenario"
        );
        fs::write(
            &receipt.path,
            format!(
                "{}\n{}\n{}\n",
                receipt.token,
                receipt.scenario_id,
                NativeScenarioBehavior::OpaqueHitTransparentPrefix.as_str()
            ),
        )
        .expect("the mismatched behavior receipt should be writable");
        assert_eq!(
            receipt.verify(),
            Err(()),
            "an existing test for another behavior must not satisfy the selected scenario"
        );
        fs::write(&receipt.path, receipt.expected_contents())
            .expect("the exact behavior receipt should be writable");
        assert_eq!(receipt.verify(), Ok(()));
    }
}
