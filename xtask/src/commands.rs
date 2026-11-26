use std::{
    path::Path,
    process::{Command, ExitCode},
};

use clap::{Args, Parser, Subcommand};

use crate::{
    dependency_health::dependency_health, doc_links::scan_doc_links,
    import_boundary::scan_import_boundary, native_windows_interactive::native_windows_interactive,
    public_api_snapshot::scan_public_api, release_docs::verify_release_docs,
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
    /// Run the native shared docking suite on an interactive Windows desktop.
    NativeWindowsInteractive,
    /// Verify changelog, release notes, README versions, and breaking inventory.
    VerifyReleaseDocs(ForwardArgs),
    /// Scan public documentation relative links.
    ScanDocLinks,
    /// Scan for disallowed import residue.
    ScanImportBoundary,
    /// Scan the retained docking API boundary.
    ScanPublicApi(ForwardArgs),
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
        XtaskCommand::NativeWindowsInteractive => native_windows_interactive(root),
        XtaskCommand::VerifyReleaseDocs(args) => verify_release_docs(root, &args.args),
        XtaskCommand::ScanDocLinks => scan_doc_links(root),
        XtaskCommand::ScanImportBoundary => scan_import_boundary(root),
        XtaskCommand::ScanPublicApi(args) => scan_public_api(root, &args.args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn verify(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["fmt", "--all", "--", "--check"])?;
    run(
        root,
        "cargo",
        &["check", "--workspace", "--all-targets", "--locked"],
    )?;
    run(
        root,
        "cargo",
        &[
            "nextest",
            "run",
            "--locked",
            "--all-features",
            "--test-threads=1",
            "-p",
            "open-gpui",
            "-p",
            "open-gpui-base",
            "-p",
            "open-gpui-docking",
            "-p",
            "open-gpui-dockspace-editor",
            "-p",
            "xtask",
        ],
    )?;
    verify_release_docs(root, &[])?;
    scan_doc_links(root)?;
    dependency_health(root)?;
    scan_import_boundary(root)?;
    scan_public_api(root, &[])
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
    let display = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");

    println!("==> {display}");
    let mut command = Command::new(program);
    command.args(args).current_dir(root);
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
