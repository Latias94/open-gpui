use std::{
    path::Path,
    process::{Command, ExitCode},
};

use clap::{Args, Parser, Subcommand};

use crate::{
    dependency_health::dependency_health, devtools::DevtoolsArgs, devtools::devtools,
    doc_links::scan_doc_links, import_boundary::scan_import_boundary,
    public_api_snapshot::scan_public_api, release_docs::verify_release_docs,
    theme_drift::scan_theme_drift, theme_schema::scan_theme_schema, ui_contract::scan_ui_contract,
    web_smoke::web_smoke,
};

#[derive(Parser, Debug)]
#[command(name = "xtask", about = "Workspace automation for Open GPUI.")]
struct XtaskCli {
    #[command(subcommand)]
    command: XtaskCommand,
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
enum XtaskCommand {
    /// Run the local Open GPUI gate.
    Verify,
    /// Verify MSRV, duplicate dependencies, and cargo audit.
    DependencyHealth,
    /// Run the complete native WGPU package test suite.
    RendererSmoke,
    /// Verify changelog, release notes, README versions, and breaking inventory.
    VerifyReleaseDocs(ForwardArgs),
    /// Scan public documentation relative links.
    ScanDocLinks,
    /// Scan theme token and recipe drift.
    ScanThemeDrift,
    /// Scan theme JSON schema artifact drift.
    ScanThemeSchema,
    /// Scan for disallowed import residue.
    ScanImportBoundary,
    /// Scan public API tier drift.
    ScanPublicApi(ForwardArgs),
    /// Scan UI component contract drift.
    ScanUiContract,
    /// Build and run the stable browser smoke test.
    WebSmoke(WebSmokeArgs),
    /// Inspect, diagnose, diff, and stream DevTools artifacts.
    Devtools(DevtoolsArgs),
}

#[derive(Args, Debug)]
struct ForwardArgs {
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    args: Vec<String>,
}

#[derive(Args, Debug)]
struct WebSmokeArgs {
    /// Allow a local diagnostic run to skip when WebGPU is unavailable.
    #[arg(long)]
    allow_unavailable: bool,
}

pub fn run_from_env() -> ExitCode {
    let cli = match XtaskCli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            return if exit_code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
    };

    let root = workspace_root();
    let result = match cli.command {
        XtaskCommand::Verify => verify(root),
        XtaskCommand::DependencyHealth => dependency_health(root),
        XtaskCommand::RendererSmoke => renderer_smoke(root),
        XtaskCommand::VerifyReleaseDocs(args) => verify_release_docs(root, &args.args),
        XtaskCommand::ScanDocLinks => scan_doc_links(root),
        XtaskCommand::ScanThemeDrift => scan_theme_drift(root),
        XtaskCommand::ScanThemeSchema => scan_theme_schema(root),
        XtaskCommand::ScanImportBoundary => scan_import_boundary(root),
        XtaskCommand::ScanPublicApi(args) => scan_public_api(root, &args.args),
        XtaskCommand::ScanUiContract => scan_ui_contract(root),
        XtaskCommand::WebSmoke(args) => web_smoke(root, args.allow_unavailable),
        XtaskCommand::Devtools(args) => devtools(root, args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyHost {
    Windows,
    Other,
}

impl VerifyHost {
    fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerifyCommand {
    program: &'static str,
    args: Vec<&'static str>,
    environment: Vec<(&'static str, &'static str)>,
}

impl VerifyCommand {
    fn cargo(args: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            program: "cargo",
            args: args.into_iter().collect(),
            environment: Vec::new(),
        }
    }

    fn with_environment(mut self, key: &'static str, value: &'static str) -> Self {
        self.environment.push((key, value));
        self
    }

    fn run(&self, root: &Path) -> Result<(), ()> {
        run_with_environment(root, self.program, &self.args, self.environment.as_slice())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyGate {
    ReleaseDocs,
    DocLinks,
    DependencyHealth,
    ThemeDrift,
    ThemeSchema,
    ImportBoundary,
    PublicApi,
    UiContract,
}

impl VerifyGate {
    fn run(self, root: &Path) -> Result<(), ()> {
        match self {
            Self::ReleaseDocs => verify_release_docs(root, &[]),
            Self::DocLinks => scan_doc_links(root),
            Self::DependencyHealth => dependency_health(root),
            Self::ThemeDrift => scan_theme_drift(root),
            Self::ThemeSchema => scan_theme_schema(root),
            Self::ImportBoundary => scan_import_boundary(root),
            Self::PublicApi => scan_public_api(root, &["--check".to_owned()]),
            Self::UiContract => scan_ui_contract(root),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VerifyStep {
    Command(VerifyCommand),
    Gate(VerifyGate),
}

impl VerifyStep {
    fn run(&self, root: &Path) -> Result<(), ()> {
        match self {
            Self::Command(command) => command.run(root),
            Self::Gate(gate) => gate.run(root),
        }
    }
}

fn devtools_all_features_command(
    host: VerifyHost,
    args: impl IntoIterator<Item = &'static str>,
) -> VerifyCommand {
    let command = VerifyCommand::cargo(args);
    match host {
        VerifyHost::Windows => command.with_environment("CARGO_BUILD_JOBS", "1"),
        VerifyHost::Other => command,
    }
}

fn verify_plan(host: VerifyHost) -> Vec<VerifyStep> {
    use VerifyGate as Gate;

    vec![
        VerifyStep::Command(VerifyCommand::cargo(["fmt", "--all", "--", "--check"])),
        VerifyStep::Command(VerifyCommand::cargo([
            "check",
            "--workspace",
            "--all-targets",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "check",
            "-p",
            "open-gpui-smoke-native",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "nextest",
            "run",
            "-p",
            "open-gpui-motion",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "test",
            "-p",
            "open-gpui-motion",
            "--doc",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "nextest",
            "run",
            "-p",
            "open-gpui-form",
            "-p",
            "open-gpui-resource",
            "-p",
            "open-gpui-devtools",
            "--locked",
        ])),
        VerifyStep::Command(devtools_all_features_command(
            host,
            [
                "nextest",
                "run",
                "-p",
                "open-gpui-devtools",
                "--all-features",
                "--no-fail-fast",
                "--locked",
            ],
        )),
        VerifyStep::Command(devtools_all_features_command(
            host,
            [
                "test",
                "-p",
                "open-gpui-devtools",
                "--all-features",
                "--doc",
                "--locked",
            ],
        )),
        VerifyStep::Command(VerifyCommand::cargo([
            "nextest",
            "run",
            "-p",
            "open-gpui",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "nextest",
            "run",
            "-p",
            "open-gpui",
            "--all-features",
            "--test",
            "presentation_surface",
            "--locked",
            "--no-fail-fast",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "nextest",
            "run",
            "-p",
            "open-gpui-ui-core",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "nextest",
            "run",
            "-p",
            "open-gpui-ui-components",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "nextest",
            "run",
            "-p",
            "open-gpui-ui-foundation-gallery",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "test",
            "-p",
            "open-gpui-ui-core",
            "--doc",
            "--locked",
        ])),
        VerifyStep::Command(VerifyCommand::cargo([
            "test",
            "-p",
            "open-gpui-ui-components",
            "--doc",
            "--locked",
        ])),
        VerifyStep::Gate(Gate::ReleaseDocs),
        VerifyStep::Gate(Gate::DocLinks),
        VerifyStep::Gate(Gate::DependencyHealth),
        VerifyStep::Gate(Gate::ThemeDrift),
        VerifyStep::Gate(Gate::ThemeSchema),
        VerifyStep::Gate(Gate::ImportBoundary),
        VerifyStep::Gate(Gate::PublicApi),
        VerifyStep::Gate(Gate::UiContract),
    ]
}

fn verify(root: &Path) -> Result<(), ()> {
    for step in verify_plan(VerifyHost::current()) {
        step.run(root)?;
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
            "--locked",
            "--no-fail-fast",
        ],
    )
}

pub(crate) fn run(root: &Path, program: &str, args: &[&str]) -> Result<(), ()> {
    run_with_environment(root, program, args, &[])
}

fn run_with_environment(
    root: &Path,
    program: &str,
    args: &[&str],
    environment: &[(&str, &str)],
) -> Result<(), ()> {
    let display = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");

    println!("==> {display}");
    let mut command = Command::new(program);
    command
        .args(args)
        .envs(environment.iter().copied())
        .current_dir(root);
    let status = command.status().map_err(|error| {
        eprintln!("failed to run `{display}`: {error}");
    })?;

    if status.success() {
        Ok(())
    } else {
        eprintln!("command failed: {display}");
        Err(())
    }
}

pub(crate) fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest should live under the workspace root")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cargo_commands(plan: &[VerifyStep]) -> Vec<&VerifyCommand> {
        plan.iter()
            .filter_map(|step| match step {
                VerifyStep::Command(command) if command.program == "cargo" => Some(command),
                VerifyStep::Command(_) | VerifyStep::Gate(_) => None,
            })
            .collect()
    }

    fn is_devtools_all_features(command: &&VerifyCommand) -> bool {
        command.args.windows(2).any(|args| {
            args == ["-p", "open-gpui-devtools"] || args == ["--package", "open-gpui-devtools"]
        }) && command.args.contains(&"--all-features")
    }

    #[test]
    fn web_smoke_requires_an_explicit_unavailable_skip() {
        let default = XtaskCli::try_parse_from(["xtask", "web-smoke"]).unwrap();
        let XtaskCommand::WebSmoke(default) = default.command else {
            panic!("web-smoke should parse as its own command");
        };
        assert!(!default.allow_unavailable);

        let local =
            XtaskCli::try_parse_from(["xtask", "web-smoke", "--allow-unavailable"]).unwrap();
        let XtaskCommand::WebSmoke(local) = local.command else {
            panic!("web-smoke should parse as its own command");
        };
        assert!(local.allow_unavailable);
    }

    #[test]
    fn verify_plan_covers_u11_targets_features_and_process_scoped_environment() {
        let windows_plan = verify_plan(VerifyHost::Windows);
        let windows_commands = cargo_commands(&windows_plan);

        assert!(windows_commands.iter().any(|command| {
            command.args == ["check", "--workspace", "--all-targets", "--locked"]
        }));
        assert_eq!(
            windows_commands
                .iter()
                .map(|command| command.args.as_slice())
                .collect::<Vec<_>>(),
            [
                &["fmt", "--all", "--", "--check"][..],
                &["check", "--workspace", "--all-targets", "--locked"][..],
                &["check", "-p", "open-gpui-smoke-native", "--locked"][..],
                &["nextest", "run", "-p", "open-gpui-motion", "--locked"][..],
                &["test", "-p", "open-gpui-motion", "--doc", "--locked"][..],
                &[
                    "nextest",
                    "run",
                    "-p",
                    "open-gpui-form",
                    "-p",
                    "open-gpui-resource",
                    "-p",
                    "open-gpui-devtools",
                    "--locked",
                ][..],
                &[
                    "nextest",
                    "run",
                    "-p",
                    "open-gpui-devtools",
                    "--all-features",
                    "--no-fail-fast",
                    "--locked",
                ][..],
                &[
                    "test",
                    "-p",
                    "open-gpui-devtools",
                    "--all-features",
                    "--doc",
                    "--locked",
                ][..],
                &["nextest", "run", "-p", "open-gpui", "--locked"][..],
                &[
                    "nextest",
                    "run",
                    "-p",
                    "open-gpui",
                    "--all-features",
                    "--test",
                    "presentation_surface",
                    "--locked",
                    "--no-fail-fast",
                ][..],
                &["nextest", "run", "-p", "open-gpui-ui-core", "--locked"][..],
                &[
                    "nextest",
                    "run",
                    "-p",
                    "open-gpui-ui-components",
                    "--locked",
                ][..],
                &[
                    "nextest",
                    "run",
                    "-p",
                    "open-gpui-ui-foundation-gallery",
                    "--locked",
                ][..],
                &["test", "-p", "open-gpui-ui-core", "--doc", "--locked"][..],
                &["test", "-p", "open-gpui-ui-components", "--doc", "--locked",][..],
            ]
        );

        for command in &windows_commands {
            let is_nextest = command.args.starts_with(&["nextest", "run"]);
            let is_doctest =
                command.args.first() == Some(&"test") && command.args.contains(&"--doc");
            if is_nextest || is_doctest {
                assert!(
                    command.args.contains(&"--locked"),
                    "verification command is not lockfile-stable: {:?}",
                    command.args
                );
            }
        }

        let windows_devtools = windows_commands
            .iter()
            .copied()
            .filter(is_devtools_all_features)
            .collect::<Vec<_>>();
        assert_eq!(windows_devtools.len(), 2);
        assert!(windows_devtools.iter().any(|command| {
            command.args
                == [
                    "nextest",
                    "run",
                    "-p",
                    "open-gpui-devtools",
                    "--all-features",
                    "--no-fail-fast",
                    "--locked",
                ]
        }));
        assert!(windows_devtools.iter().any(|command| {
            command.args
                == [
                    "test",
                    "-p",
                    "open-gpui-devtools",
                    "--all-features",
                    "--doc",
                    "--locked",
                ]
        }));
        assert!(
            windows_devtools
                .iter()
                .all(|command| { command.environment == [("CARGO_BUILD_JOBS", "1")] })
        );
        assert_eq!(
            windows_commands
                .iter()
                .filter(|command| !command.environment.is_empty())
                .count(),
            windows_devtools.len()
        );

        let other_plan = verify_plan(VerifyHost::Other);
        assert!(
            cargo_commands(&other_plan)
                .into_iter()
                .filter(is_devtools_all_features)
                .all(|command| command.environment.is_empty())
        );
        assert_eq!(
            other_plan
                .iter()
                .filter_map(|step| match step {
                    VerifyStep::Gate(gate) => Some(*gate),
                    VerifyStep::Command(_) => None,
                })
                .collect::<Vec<_>>(),
            [
                VerifyGate::ReleaseDocs,
                VerifyGate::DocLinks,
                VerifyGate::DependencyHealth,
                VerifyGate::ThemeDrift,
                VerifyGate::ThemeSchema,
                VerifyGate::ImportBoundary,
                VerifyGate::PublicApi,
                VerifyGate::UiContract,
            ]
        );
    }
}
