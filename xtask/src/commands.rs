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
    /// Run the native wgpu renderer smoke test.
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
    WebSmoke,
    /// Inspect, diagnose, diff, and stream DevTools artifacts.
    Devtools(DevtoolsArgs),
}

#[derive(Args, Debug)]
struct ForwardArgs {
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    args: Vec<String>,
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
        XtaskCommand::WebSmoke => web_smoke(root),
        XtaskCommand::Devtools(args) => devtools(root, args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn verify(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["fmt", "--all", "--check"])?;
    run(root, "cargo", &["check", "--workspace"])?;
    run(root, "cargo", &["check", "-p", "open-gpui-smoke-native"])?;
    run_motion_tests(root)?;
    run_ecosystem_tests(root)?;
    run_gpui_tests(root)?;
    run_ui_component_tests(root)?;
    verify_release_docs(root, &[])?;
    scan_doc_links(root)?;
    dependency_health(root)?;
    scan_theme_drift(root)?;
    scan_import_boundary(root)?;
    scan_public_api(root, &["--check".to_string()])?;
    scan_ui_contract(root)?;
    Ok(())
}

fn run_gpui_tests(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["nextest", "run", "-p", "open-gpui"])?;
    Ok(())
}

fn run_motion_tests(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["nextest", "run", "-p", "open-gpui-motion"])?;
    run(root, "cargo", &["test", "-p", "open-gpui-motion", "--doc"])?;
    Ok(())
}

fn run_ecosystem_tests(root: &Path) -> Result<(), ()> {
    run(
        root,
        "cargo",
        &[
            "nextest",
            "run",
            "-p",
            "open-gpui-form",
            "-p",
            "open-gpui-resource",
            "-p",
            "open-gpui-devtools",
        ],
    )?;
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

    for package in ["open-gpui-ui-core", "open-gpui-ui-components"] {
        run(root, "cargo", &["test", "-p", package, "--doc"])?;
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

pub(crate) fn run(root: &Path, program: &str, args: &[&str]) -> Result<(), ()> {
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

pub(crate) fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest should live under the workspace root")
}
