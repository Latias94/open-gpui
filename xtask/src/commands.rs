use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use crate::{
    import_boundary::scan_import_boundary, theme_drift::scan_theme_drift,
    theme_schema::scan_theme_schema, ui_contract::scan_ui_contract,
};

pub fn run_from_env() -> ExitCode {
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
        "scan-theme-schema" => scan_theme_schema(&root),
        "scan-import-boundary" => scan_import_boundary(&root),
        "scan-ui-contract" => scan_ui_contract(&root),
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
    eprintln!("  scan-theme-schema     scan theme JSON schema artifact drift");
    eprintln!("  scan-import-boundary  scan for disallowed import residue");
    eprintln!("  scan-ui-contract      scan UI component contract drift");
}

fn verify(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["fmt", "--all", "--check"])?;
    run(root, "cargo", &["check", "--workspace"])?;
    run(root, "cargo", &["check", "-p", "open-gpui-smoke-native"])?;
    run_motion_tests(root)?;
    run_ui_component_tests(root)?;
    scan_theme_drift(root)?;
    scan_import_boundary(root)?;
    scan_ui_contract(root)?;
    Ok(())
}

fn run_motion_tests(root: &Path) -> Result<(), ()> {
    run(root, "cargo", &["nextest", "run", "-p", "open-gpui-motion"])?;
    run(root, "cargo", &["test", "-p", "open-gpui-motion", "--doc"])?;
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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest should live under the workspace root")
        .to_path_buf()
}
